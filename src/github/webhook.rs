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
use crate::state::{PrMessage, PrMessageStore, SubscriptionStore};

type HmacSha256 = Hmac<Sha256>;

/// State shared across all webhook handler invocations.
#[derive(Clone)]
pub struct WebhookState {
    /// HMAC secret for verifying GitHub payloads
    pub secret: String,

    /// Serenity HTTP client for posting Discord messages
    pub http: Arc<Http>,

    /// Persistent store for PR message IDs
    pub pr_store: Arc<dyn PrMessageStore>,

    /// Persistent store for channel subscription
    pub sub_store: Arc<dyn SubscriptionStore>,
}

// ── Handler ───────────────────────────────────────────────────────────────────

/// Axum handler for `POST /github/webhook`.
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

    let subscriptions = state
        .sub_store
        .get_all_for_repo(&payload.repository.full_name)
        .await?;

    if subscriptions.is_empty() {
        info!(repo = %payload.repository.full_name, "no subscriptions found");
        return Ok(StatusCode::OK.into_response());
    }

    match payload.action.as_str() {
        "opened" => {
            for sub in &subscriptions {
                let channel_id = ChannelId::from(sub.channel_id);
                let message_id =
                    messages::post_pull_request(&state.http, channel_id, &payload).await?;

                state
                    .pr_store
                    .upsert(PrMessage {
                        repo: payload.repository.full_name.clone(),
                        pr_number: pr.number,
                        channel_id: sub.channel_id,
                        message_id,
                    })
                    .await?;
            }
        }
        "closed" | "reopened" | "synchronize" => {
            let record = state
                .pr_store
                .get(&payload.repository.full_name, pr.number)
                .await?;

            if let Some(record) = record {
                messages::update_pull_request(
                    &state.http,
                    ChannelId::new(record.channel_id),
                    record.message_id,
                    &payload,
                )
                .await?;

                if payload.action == "closed" {
                    state
                        .pr_store
                        .delete(&payload.repository.full_name, pr.number)
                        .await?;
                }
            } else {
                info!(pr = pr.number, "no stored message for PR — skipping update");
            }
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
        let record = state
            .pr_store
            .get(&payload.repository.full_name, pr.number)
            .await?;

        if let Some(record) = record {
            messages::post_review(&state.http, ChannelId::new(record.channel_id), &payload).await?;
        }
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
