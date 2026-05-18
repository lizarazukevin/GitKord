//! GitHub REST API helpers.
//!
//! All functions take an `Octocrab` instance built from `GITHUB_TOKEN`.
//! Build the client once at startup and pass it through shared state.

use octocrab::models::hooks::{Config as HookConfig, ContentType as HookContentType, Hook};
use octocrab::models::webhook_events::WebhookEventType;
use octocrab::Octocrab;
use tracing::info;

use crate::error::AppError;
use crate::error::Result;

/// Build an authenticated `Octocrab` client from a personal access token.
///
/// # Errors
///
/// Returns [`AppError::GitHub`] if the client cannot be initialised.
pub fn build_client(token: &str) -> Result<Octocrab> {
    Octocrab::builder()
        .personal_token(token.to_owned())
        .build()
        .map_err(AppError::GitHub)
}

/// Verify that a GitHub username exists and return their login.
///
/// Returns `None` on `404` so caller gives a friendly error instead of persisting
/// a username that does not exist.
///
/// # Errors
///
/// Returns [`AppError::GitHub`] on network or non-404 API errors.
pub async fn verify_user(client: &Octocrab, username: &str) -> Result<Option<String>> {
    match client.users(username).profile().await {
        Ok(user) => {
            info!(username, "GitHub user verified");
            Ok(Some(user.login))
        }
        Err(octocrab::Error::GitHub { source, .. }) if source.status_code == 404 => Ok(None),
        Err(e) => Err(AppError::GitHub(e)),
    }
}

/// Register a webhook on a GitHub repository.
///
/// Returns the hook ID if one was created, `None` if it already existed (422).
///
/// # Errors
///
/// Returns [`AppError::GitHub`] if the API call fails reasons other than hook already existing.
pub async fn register_webhook(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    payload_url: &str,
    secret: &str,
) -> Result<Option<u64>> {
    let config = HookConfig {
        url: payload_url.to_owned(),
        content_type: Some(HookContentType::Json),
        secret: Some(secret.to_owned()),
        insecure_ssl: None,
    };

    let hook = Hook {
        name: "web".to_owned(),
        config,
        events: vec![
            WebhookEventType::PullRequest,
            WebhookEventType::PullRequestReview,
            WebhookEventType::Push,
        ],
        active: true,
        ..Hook::default()
    };

    match client.repos(owner, repo).create_hook(hook).await {
        Ok(created) => {
            let id = created.id;
            info!(owner, repo, hook_id = id, "webhook registered");
            Ok(Some(id))
        }

        Err(octocrab::Error::GitHub { source, .. }) if source.status_code == 422 => {
            info!(owner, repo, "webhook already registered, skipping");
            Ok(None)
        }
        Err(e) => Err(AppError::GitHub(e)),
    }
}

/// Request a review from a GitHub user on a pull request.
///
/// # Errors
///
/// Returns [`AppError::GitHub`] if the API call fails or the reviewer
/// does not have access to the repository.
pub async fn assign_reviewer(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    pr_number: u64,
    reviewer: &str,
) -> Result<()> {
    client
        .pulls(owner, repo)
        .request_reviews(pr_number, vec![reviewer.to_owned()], vec![])
        .await
        .map_err(AppError::GitHub)?;

    info!(owner, repo, pr_number, reviewer, "reviewer assigned");
    Ok(())
}

/// Remove a review request from a GitHub user on a pull request.
///
/// # Errors
///
/// Returns [`AppError::GitHub`] if the API call fails.
pub async fn unassign_reviewer(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    pr_number: u64,
    reviewer: &str,
) -> Result<()> {
    client
        .pulls(owner, repo)
        .remove_requested_reviewers(pr_number, vec![reviewer.to_owned()], vec![])
        .await
        .map_err(AppError::GitHub)?;

    info!(owner, repo, pr_number, reviewer, "reviewer unassigned");
    Ok(())
}
