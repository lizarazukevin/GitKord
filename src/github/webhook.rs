//! `GitHub` webhook endpoint.
//!
//! All requests hit `verify_signature` before any payload parsing happens.
//! Invalid signatures are rejected with `401 Unauthorized` before we touch the body.

use axum::response::Response;
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use hmac::{Hmac, KeyInit, Mac};
use serenity::all::ChannelId;
use sha2::Sha256;
use tracing::{info, warn};

use crate::discord::messages;
use crate::error::AppError;
use crate::error::Result;
use crate::github::api;
pub use crate::github::context::WebhookState;
use crate::github::payloads::{
    GitHubEvent, PullRequestPayload, PullRequestReviewPayload, PushPayload,
};
use crate::state::PrMessage;

/// Entry point for `POST /github/webhook`.
pub async fn handle(
    State(state): State<WebhookState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response> {
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

async fn handle_pull_request(state: WebhookState, payload: PullRequestPayload) -> Result<Response> {
    let pr = &payload.pull_request;

    info!(
        repo   = %payload.repository.full_name,
        pr     = pr.number,
        action = %payload.action,
        status = %pr.status_label(),
        title  = %pr.title,
        "pull_request event"
    );

    let subscriptions = state
        .sub_store
        .get_all_for_repo(&payload.repository.full_name)
        .await?;

    if subscriptions.is_empty() {
        info!(repo = %payload.repository.full_name, "no subscriptions found, skipping");
        return Ok(StatusCode::OK.into_response());
    }

    let Some((owner, repo_name)) = payload.repository.full_name.split_once('/') else {
        tracing::error!(repo = %payload.repository.full_name, "malformed repository full_name");
        return Ok(StatusCode::OK.into_response());
    };

    let message_data = api::fetch_pr_message_data(
        &state.github,
        owner,
        repo_name,
        &payload.repository.full_name,
        &payload.pull_request,
    )
    .await?;

    match payload.action.as_str() {
        "opened" => {
            for sub in &subscriptions {
                let channel_id = ChannelId::from(sub.channel_id);
                let (message_id, thread_id) =
                    messages::post_pull_request(&state.http, channel_id, &payload, &message_data)
                        .await?;

                state
                    .pr_store
                    .upsert(PrMessage {
                        repo: payload.repository.full_name.clone(),
                        pr_number: pr.number,
                        channel_id: sub.channel_id,
                        message_id,
                        thread_id,
                    })
                    .await?;
            }
        }
        "review_requested" | "review_request_removed" => {
            let record = state
                .pr_store
                .get(&payload.repository.full_name, pr.number)
                .await?;

            if let Some(record) = record {
                messages::update_pull_request(
                    &state.http,
                    ChannelId::new(record.channel_id),
                    record.message_id,
                    record.thread_id,
                    pr.number,
                    &payload.action,
                    &message_data,
                )
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
                    record.thread_id,
                    pr.number,
                    &payload.action,
                    &message_data,
                )
                .await?;

                if payload.action == "closed" {
                    state
                        .pr_store
                        .delete(&payload.repository.full_name, pr.number)
                        .await?;
                }
            } else {
                info!(pr = pr.number, "no stored message found, skipping update");
            }
        }
        _ => {}
    }

    Ok(StatusCode::OK.into_response())
}

async fn handle_pull_request_review(
    state: WebhookState,
    payload: PullRequestReviewPayload,
) -> Result<Response> {
    let pr = &payload.pull_request;
    let review = &payload.review;

    info!(
        repo     = %payload.repository.full_name,
        pr       = pr.number,
        action   = %payload.action,
        reviewer = %review.user.login,
        verdict  = %review.verdict_emoji(),
        "pull_request_review event"
    );

    if payload.action == "submitted" || payload.action == "dismissed" {
        let record = state
            .pr_store
            .get(&payload.repository.full_name, pr.number)
            .await?;

        if let Some(record) = record {
            let Some((owner, repo_name)) = payload.repository.full_name.split_once('/') else {
                tracing::error!(repo = %payload.repository.full_name, "malformed repository full_name");
                return Ok(StatusCode::OK.into_response());
            };

            let message_data = api::fetch_pr_message_data(
                &state.github,
                owner,
                repo_name,
                &payload.repository.full_name,
                &payload.pull_request,
            )
            .await?;

            messages::update_pull_request(
                &state.http,
                ChannelId::new(record.channel_id),
                record.message_id,
                record.thread_id,
                pr.number,
                &payload.action,
                &message_data,
            )
            .await?;

            messages::post_review(&state.http, record.thread_id, &payload).await?;
        }
    }

    Ok(StatusCode::OK.into_response())
}

fn handle_push(payload: &PushPayload) -> StatusCode {
    if payload.git_ref == "refs/heads/main" {
        info!(
            repo    = %payload.repository.full_name,
            sha     = &payload.after[..7],
            commits = payload.commits.len(),
            "push to main"
        );
    }
    StatusCode::OK
}

// ── Signature verification ────────────────────────────────────────────────────
pub type HmacSha256 = Hmac<Sha256>;

/// Verify the `X-Hub-Signature-256` against the raw request body using `HMAC-SHA256`.
///
/// Compute and compare in constant time to prevent timing attacks.
fn verify_signature(secret: &str, headers: &HeaderMap, body: &Bytes) -> Result<()> {
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
