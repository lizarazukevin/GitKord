#![allow(unused)]

//! GitHub REST API helpers via `octocrab`.
//!
//! All functions take an `octocrab::Octocrab` instance built from the
//! `GITHUB_TOKEN` PAT. Callers are responsible for constructing the client
//! once at startup and passing it through shared state.

use octocrab::models::hooks::{Config as HookConfig, ContentType as HookContentType, Hook};
use octocrab::models::webhook_events::WebhookEventType;
use octocrab::Octocrab;
use tracing::info;

use crate::error::AppError;

// ── Client builder ────────────────────────────────────────────────────────────

/// Build an authenticated `Octocrab` client from a personal access token.
///
/// # Errors
///
/// Returns [`AppError::GitHub`] if the client cannot be initialised.
pub fn build_client(token: &str) -> Result<Octocrab, AppError> {
    Octocrab::builder()
        .personal_token(token.to_owned())
        .build()
        .map_err(AppError::GitHub)
}

// ── User verification ─────────────────────────────────────────────────────────

/// Verify that a GitHub username exists and return their login.
///
/// Used by `/link` to confirm the username is real before persisting it.
/// Returns `None` if the user does not exist (404).
///
/// # Errors
///
/// Returns [`AppError::GitHub`] on network or API errors other than 404.
pub async fn verify_user(client: &Octocrab, username: &str) -> Result<Option<String>, AppError> {
    match client.users(username).profile().await {
        Ok(user) => {
            info!(username, "GitHub user verified");
            Ok(Some(user.login))
        }
        Err(octocrab::Error::GitHub { source, .. }) if source.status_code == 404 => Ok(None),
        Err(e) => Err(AppError::GitHub(e)),
    }
}

// ── Webhook registration ──────────────────────────────────────────────────────

/// Register a webhook on a GitHub repository.
///
/// Silently succeeds if the webhook already exists (422). Returns the hook ID
/// if one was created, or `None` if it already existed.
///
/// # Errors
///
/// Returns [`AppError::GitHub`] if the API call fails for any other reason.
pub async fn register_webhook(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    payload_url: &str,
    secret: &str,
) -> Result<Option<u64>, AppError> {
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
        // 422 = hook already exists — treat as success
        Err(octocrab::Error::GitHub { source, .. }) if source.status_code == 422 => {
            info!(owner, repo, "webhook already registered — skipping");
            Ok(None)
        }
        Err(e) => Err(AppError::GitHub(e)),
    }
}
