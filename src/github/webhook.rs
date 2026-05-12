//! GitHub webhook endpoint.
//!
//! Handles `POST /github/webhook`. Every incoming request is verified against
//! the HMAC-SHA256 signature in `X-Hub-Signature-256` before the payload is
//! parsed — unauthenticated requests are rejected with `401 Unauthorized`.

use std::sync::Arc;

use axum::response::Response;
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use hmac::{Hmac, KeyInit, Mac};
use serenity::all::{ChannelId, Http};
use sha2::Sha256;
use tracing::{info, warn};

use crate::discord::messages;
use crate::error::AppError;
use crate::github::types::{
    GitHubEvent, PullRequestPayload, PullRequestReviewPayload, PushPayload,
};

type HmacSha256 = Hmac<Sha256>;

/// State shared across all webhook handler invocations.
#[derive(Clone)]
pub struct WebhookState {
    /// HMAC secret for verifying GitHub payloads
    pub secret: String,

    /// Serenity HTTP client for posting Discord messages
    pub http: Arc<Http>,

    /// Channel to post PR messages to (temporary)
    pub channel_id: ChannelId,
}

// ── Handler ───────────────────────────────────────────────────────────────────

/// Axum handler for `POST /github/webhook`.
///
/// 1. Reads the raw request body as bytes (required for signature verification).
/// 2. Verifies the `X-Hub-Signature-256` header against the shared secret.
/// 3. Dispatches to the appropriate event handler based on `X-GitHub-Event`.
pub async fn handle(
    State(state): State<WebhookState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    verify_signature(&state.secret, &headers, &body)?;

    let event_type = headers
        .get("x-github-event")
        .and_then(|v| v.to_str().ok())
        .map_or_else(
            || GitHubEvent::Unknown("missing header".into()),
            GitHubEvent::from,
        );

    match event_type {
        GitHubEvent::Ping => {
            info!("GitHub ping received — webhook is connected");
            Ok(StatusCode::OK.into_response())
        }

        GitHubEvent::PullRequest => {
            let payload: PullRequestPayload =
                serde_json::from_slice(&body).map_err(|e| AppError::Internal(e.into()))?;
            handle_pull_request(state, payload).await
        }

        GitHubEvent::PullRequestReview => {
            let payload: PullRequestReviewPayload =
                serde_json::from_slice(&body).map_err(|e| AppError::Internal(e.into()))?;
            handle_pull_request_review(state, payload).await
        }

        GitHubEvent::Push => {
            let payload: PushPayload =
                serde_json::from_slice(&body).map_err(|e| AppError::Internal(e.into()))?;
            Ok(handle_push(&payload).into_response())
        }

        GitHubEvent::Unknown(name) => {
            info!(event = %name, "ignoring unhandled event type");
            Ok(StatusCode::OK.into_response())
        }
    }
}

// ── Event handlers ────────────────────────────────────────────────────────────

async fn handle_pull_request(
    state: WebhookState,
    payload: PullRequestPayload,
) -> Result<Response, AppError> {
    let pr = &payload.pull_request;

    info!(
        repo   = %payload.repository.full_name,
        pr     = pr.number,
        action = %payload.action,
        status = %pr.status_label(),
        title  = %pr.title,
        "pull_request event received"
    );

    match payload.action.as_str() {
        "opened" => {
            messages::post_pull_request(&state.http, state.channel_id, &payload).await?;
        }
        "closed" | "reopened" | "synchronize" => {
            info!(action = %payload.action, "update not yet persisted, state layer pending");
        }
        _ => {}
    }

    Ok(StatusCode::OK.into_response())
}

async fn handle_pull_request_review(
    state: WebhookState,
    payload: PullRequestReviewPayload,
) -> Result<Response, AppError> {
    let pr = &payload.pull_request;
    let review = &payload.review;

    info!(
        repo     = %payload.repository.full_name,
        pr       = pr.number,
        action   = %payload.action,
        reviewer = %review.user.login,
        verdict  = %review.verdict_emoji(),
        "pull_request_review event received"
    );

    if payload.action == "submitted" {
        messages::post_review(&state.http, state.channel_id, &payload).await?;
    }

    Ok(StatusCode::OK.into_response())
}

fn handle_push(payload: &PushPayload) -> StatusCode {
    // Only care about pushes to the default branch — merges, hotfixes, etc.
    if payload.git_ref == "refs/heads/main" {
        info!(
            repo    = %payload.repository.full_name,
            sha     = &payload.after[..7], // short SHA
            commits = payload.commits.len(),
            "push to main received"
        );
    }
    StatusCode::OK
}

// ── Signature verification ────────────────────────────────────────────────────

/// Verify the `X-Hub-Signature-256` header against the raw request body.
///
/// GitHub computes `HMAC-SHA256(secret, body)` and sends it as
/// `sha256=<hex_digest>`. We recompute and compare in constant time to
/// prevent timing attacks.
fn verify_signature(secret: &str, headers: &HeaderMap, body: &Bytes) -> Result<(), AppError> {
    let signature = headers
        .get("x-hub-signature-256")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("sha256="))
        .ok_or(AppError::InvalidSignature)?;

    let sig_bytes = hex::decode(signature).map_err(|_| AppError::InvalidSignature)?;

    let mut mac: HmacSha256 =
        KeyInit::new_from_slice(secret.as_bytes()).expect("HMAC accepts keys of any length");
    mac.update(body);

    mac.verify_slice(&sig_bytes).map_err(|_| {
        warn!("webhook signature mismatch — request rejected");
        AppError::InvalidSignature
    })
}
