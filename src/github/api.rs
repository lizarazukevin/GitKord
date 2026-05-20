//! GitHub REST API helpers.
//!
//! All functions take an `Octocrab` instance built from `GITHUB_TOKEN`.
//! Build the client once at startup and pass it through shared state.

use crate::error::AppError;
use crate::error::Result;
use crate::github::models::{PrMessageData, ReviewState, ReviewSummary};
use crate::github::payloads::PullRequestPayload;
use octocrab::models::hooks::{Config as HookConfig, ContentType as HookContentType, Hook};
use octocrab::models::webhook_events::WebhookEventType;
use octocrab::Octocrab;
use std::collections::HashMap;
use tracing::info;

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

/// Fetch PR details and reviewer status used to capture current state.
///
/// # Errors
///
/// Returns [`AppError::GitHub`] if the API call fails
pub async fn fetch_pr_message_data(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    pr_number: u64,
    payload: &PullRequestPayload,
) -> Result<PrMessageData> {
    let pr = client
        .pulls(owner, repo)
        .get(pr_number)
        .await
        .map_err(AppError::GitHub)?;

    let reviewers = client
        .pulls(owner, repo)
        .list_reviews(pr_number)
        .send()
        .await
        .map_err(AppError::GitHub)?;

    // Track the latest verdict per reviewer. GitHub returns reviews in
    // chronological order so overwriting gives us the current state naturally.
    let mut latest: HashMap<String, ReviewState> = HashMap::new();

    for review in reviewers {
        let login = review.user.map(|u| u.login).unwrap_or_default();
        if login.is_empty() {
            continue;
        }
        let state = match review.state {
            Some(octocrab::models::pulls::ReviewState::Approved) => ReviewState::Approved,
            Some(octocrab::models::pulls::ReviewState::ChangesRequested) => ReviewState::ChangesRequested,
            // Dismissed means they need to re-review, treat as pending
            Some(octocrab::models::pulls::ReviewState::Dismissed) => ReviewState::Pending,
            _ => ReviewState::Commented,
        };
        latest.insert(login, state);
    }

    // Request to review sent, waiting to accept treated as pending
    for user in pr.requested_reviewers {
        latest.entry(user.login).or_insert(ReviewState::Pending);
    }

    let reviews = latest
        .into_iter()
        .map(|(github_login, state)| ReviewSummary {
            github_login,
            discord_tag: None,
            state,
        })
        .collect();

    Ok(PrMessageData {
        status_emoji: payload.pull_request.status_emoji(),
        number: payload.pull_request.number,
        title: payload.pull_request.title.clone(),
        author: payload.pull_request.user.login.clone(),
        repo: payload.repository.full_name.clone(),
        head: payload.pull_request.head.branch.clone(),
        base: payload.pull_request.base.branch.clone(),
        url: payload.pull_request.html_url.clone(),
        additions: pr.additions,
        deletions: pr.deletions,
        files: pr.changed_files,
        commits: pr.commits,
        comments: pr.comments,
        reviews,
        checks: vec![],
    })
}
