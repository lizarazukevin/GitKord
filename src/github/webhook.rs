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

use crate::db::models::PrChannelMessage;
use crate::discord::messages;
use crate::error::AppError;
use crate::error::Result;
use crate::github::api;
use crate::github::client::installation_client_from_id;
pub use crate::github::context::WebhookState;
use crate::github::payloads::{
    GitHubEvent, GitHubUser, InstallationPayload, IssueCommentPayload, PullRequest,
    PullRequestPayload, PullRequestRef, PullRequestReviewPayload, PushPayload,
};

/// Axum entry point for `POST /github/webhook`.
/// Verifies the HMAC signature before touching the body, then deserializes
/// and dispatches to the appropriate handler based on X-GitHub-Event.
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

        GitHubEvent::PullRequest => on_pull_request(state, deserialize(&body)?).await,
        GitHubEvent::PullRequestReview => on_pull_request_review(state, deserialize(&body)?).await,
        GitHubEvent::IssueComment => on_issue_comment(state, deserialize(&body)?).await,
        GitHubEvent::Push => Ok(on_push(&deserialize(&body)?).into_response()),

        GitHubEvent::Installation => {
            let payload: InstallationPayload = deserialize(&body)?;
            on_installation(state, payload).await
        }

        GitHubEvent::Unknown(name) => {
            info!(event = %name, "ignoring unhandled event type");
            Ok(StatusCode::OK.into_response())
        }
    }
}

/// Routes `pull_request` actions to the right internal handler.
/// opened -> post new message, `synchronize/review_requested` -> update in place,
/// closed/reopened -> lifecycle change with audit entry.
async fn on_pull_request(state: WebhookState, payload: PullRequestPayload) -> Result<Response> {
    let pr = &payload.pull_request;

    info!(
        repo   = %payload.repository.full_name,
        pr     = pr.number,
        action = %payload.action,
        status = %pr.status_label(),
        title  = %pr.title,
        "pull_request event"
    );

    match payload.action.as_str() {
        "opened" => on_pr_opened(&state, &payload).await?,
        "review_requested" | "review_request_removed" | "synchronize" => {
            on_pr_message_update(&state, &payload).await?;
        }
        "closed" | "reopened" => on_pr_lifecycle_change(&state, &payload).await?,
        _ => {}
    }

    Ok(StatusCode::OK.into_response())
}

/// Handles submitted and dismissed review events.
/// Updates the PR message to reflect the new reviewer verdict and posts
/// the review to the audit thread.
async fn on_pull_request_review(
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
        let records = broadcast_pr_update(
            &state,
            &payload.repository.owner.login,
            &payload.repository.name,
            &payload.repository.full_name,
            &payload.pull_request,
        )
        .await?;

        for record in &records {
            messages::post_review(&state.http, record.thread_id, &payload).await?;
        }
    }

    Ok(StatusCode::OK.into_response())
}

/// Handles new comments posted directly on the PR conversation (not reviews).
/// Fetches the latest PR db from GitHub and broadcasts an update to all
/// channels so the comment count stays accurate.
async fn on_issue_comment(state: WebhookState, payload: IssueCommentPayload) -> Result<Response> {
    // issue_comment fires for both issues and PRs — ignore pure issues
    if payload.issue.pull_request.is_none() || payload.action != "created" {
        return Ok(StatusCode::OK.into_response());
    }

    let pr_number = payload.issue.number;

    info!(
        repo   = %payload.repository.full_name,
        pr     = pr_number,
        action = %payload.action,
        "issue_comment event on PR"
    );

    let Some(installation_id) = state
        .sub_store
        .get_installation_id(&payload.repository.full_name)
        .await?
    else {
        info!(repo = %payload.repository.full_name, "no subscriptions found, skipping issue_comment");
        return Ok(StatusCode::OK.into_response());
    };
    let installation = installation_client_from_id(&state.github, installation_id)?;

    let pr = installation
        .pulls(&payload.repository.owner.login, &payload.repository.name)
        .get(pr_number)
        .await
        .map_err(AppError::GitHub)?;

    let pr_ref = PullRequest {
        number: pr_number,
        title: pr.title.clone(),
        state: format!("{:?}", pr.state).to_lowercase(),
        merged: Some(pr.merged),
        html_url: pr.html_url.clone().to_string(),
        user: GitHubUser {
            login: pr.user.login,
            id: pr.user.id.0,
        },
        head: PullRequestRef {
            branch: pr.head.ref_field.clone(),
            sha: pr.head.sha.clone(),
        },
        base: PullRequestRef {
            branch: pr.base.ref_field.clone(),
            sha: pr.base.sha.clone(),
        },
    };

    let records = broadcast_pr_update(
        &state,
        &payload.repository.owner.login,
        &payload.repository.name,
        &payload.repository.full_name,
        &pr_ref,
    )
    .await?;

    if records.is_empty() {
        info!(pr = pr_number, "no stored messages found, skipping");
    }

    Ok(StatusCode::OK.into_response())
}

/// Logs pushes to main. Reserved for updating the commit line on open PRs
/// once that feature is implemented.
fn on_push(payload: &PushPayload) -> StatusCode {
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

/// Called when a PR is first opened.
/// Fetches full PR data from `GitHub`, posts to every subscribed channel,
/// and stores the message and thread IDs for future updates.
async fn on_pr_opened(state: &WebhookState, payload: &PullRequestPayload) -> Result<()> {
    let pr = &payload.pull_request;
    let repo_full = &payload.repository.full_name;

    let Some(installation_id) = state.sub_store.get_installation_id(repo_full).await? else {
        info!(repo = %repo_full, "no subscriptions found, skipping");
        return Ok(());
    };
    let installation = installation_client_from_id(&state.github, installation_id)?;

    let message_data = api::assemble_pr_view(
        &installation,
        state.user_store.as_ref(),
        &payload.repository.owner.login,
        &payload.repository.name,
        repo_full,
        pr,
    )
    .await?;

    let subscriptions = state.sub_store.get_all_for_repo(repo_full).await?;

    if subscriptions.is_empty() {
        info!(repo = %payload.repository.full_name, "no subscriptions found, skipping");
        return Ok(());
    }

    for sub in subscriptions {
        let channel_id = ChannelId::from(sub.channel_id);
        let posted_pr =
            messages::post_pull_request_message(&state.http, channel_id, &message_data).await?;

        state
            .pr_store
            .upsert(PrChannelMessage {
                repo: payload.repository.full_name.clone(),
                pr_number: pr.number,
                channel_id: sub.channel_id,
                message_id: posted_pr.message_id,
                thread_id: posted_pr.thread_id,
            })
            .await?;
    }

    Ok(())
}

/// Refreshes the PR message across all subscribed channels without posting
/// an audit entry. Used for reviewer assignment changes and force pushes.
async fn on_pr_message_update(state: &WebhookState, payload: &PullRequestPayload) -> Result<()> {
    broadcast_pr_update(
        state,
        &payload.repository.owner.login,
        &payload.repository.name,
        &payload.repository.full_name,
        &payload.pull_request,
    )
    .await?;
    Ok(())
}

/// Refreshes the PR message and posts an audit entry to each thread.
/// On closed, cleans up all stored message records for the PR.
async fn on_pr_lifecycle_change(state: &WebhookState, payload: &PullRequestPayload) -> Result<()> {
    let pr = &payload.pull_request;
    let records = broadcast_pr_update(
        state,
        &payload.repository.owner.login,
        &payload.repository.name,
        &payload.repository.full_name,
        &payload.pull_request,
    )
    .await?;

    for record in &records {
        messages::post_pr_update(&state.http, record.thread_id, pr.number, &payload.action).await?;
    }

    Ok(())
}

async fn on_installation(state: WebhookState, payload: InstallationPayload) -> Result<Response> {
    match payload.action.as_str() {
        "created" => {
            info!(
                installation_id = payload.installation.id.0,
                account = %payload.installation.account.login,
                "app installed"
            );
        }
        "deleted" => {
            info!(
                installation_id = payload.installation.id.0,
                account = %payload.installation.account.login,
                "app uninstalled — cleaning up subscriptions"
            );

            for repo in &payload.repositories {
                if let Err(e) = state.sub_store.delete_all_for_repo(&repo.full_name).await {
                    tracing::error!(error = %e, repo = %repo.full_name, "failed to clean up subscriptions");
                }
            }
        }
        _ => {}
    }

    Ok(StatusCode::OK.into_response())
}

// ── Helpers ────────────────────────────────────────────────────
pub type HmacSha256 = Hmac<Sha256>;

/// Verifies X-Hub-Signature-256 against the raw body using HMAC-SHA256.
/// Constant-time comparison prevents timing attacks.
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

fn deserialize<T: serde::de::DeserializeOwned>(body: &Bytes) -> Result<T> {
    serde_json::from_slice(body).map_err(|e| AppError::Internal(e.into()))
}

/// Fetches fresh PR data and edits the message in every channel that has
/// a stored record for this PR. Returns the records so callers can post
/// audit entries if needed.
async fn broadcast_pr_update(
    state: &WebhookState,
    owner: &str,
    repo_name: &str,
    repo_full: &str,
    pr: &PullRequest,
) -> Result<Vec<PrChannelMessage>> {
    let installation_id = state.sub_store.get_installation_id(repo_full).await?;
    let Some(installation) = installation_id
        .map(|id| installation_client_from_id(&state.github, id))
        .transpose()?
    else {
        info!(repo = %repo_full, "no subscriptions found, skipping broadcast");
        return Ok(vec![]);
    };

    let message_data = api::assemble_pr_view(
        &installation,
        state.user_store.as_ref(),
        owner,
        repo_name,
        repo_full,
        pr,
    )
    .await?;

    let records = state
        .pr_store
        .get_all_by_repo_and_pr(repo_full, pr.number)
        .await?;

    if records.is_empty() {
        info!(repo = %repo_full, pr = %pr.number, "no stored messages, skipping broadcast");
        return Ok(records);
    }

    for record in &records {
        messages::update_pull_request_message(
            &state.http,
            ChannelId::from(record.channel_id),
            record.message_id,
            &message_data,
        )
        .await?;
    }

    Ok(records)
}
