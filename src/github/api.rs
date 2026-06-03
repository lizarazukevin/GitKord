//! GitHub REST API helpers.
//!
//! All functions take an `Octocrab` instance scoped to a specific installation.
//! Build the base app client at startup, then derive installation-scoped
//! clients via [`crate::github::client::installation_client_from_id`].

use crate::db::UserLinkStore;
use crate::error::AppError;
use crate::error::Result;
use crate::github::models::{PrMessageData, ReviewState, ReviewSummary};
use crate::github::payloads::PullRequest;
use indexmap::IndexMap;
use octocrab::Octocrab;
use tracing::info;

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

/// Builds message content from fetching PR details and reviewer
/// status used to snapshot its current db.
///
/// # Errors
///
/// Returns [`AppError::GitHub`] if the API call fails
pub async fn assemble_pr_view(
    client: &Octocrab,
    user_store: &dyn UserLinkStore,
    owner: &str,
    repo: &str,
    full_name: &str,
    pr_ref: &PullRequest,
) -> Result<PrMessageData> {
    let pr = client
        .pulls(owner, repo)
        .get(pr_ref.number)
        .await
        .map_err(AppError::GitHub)?;

    let raw_reviewers = client
        .pulls(owner, repo)
        .list_reviews(pr_ref.number)
        .send()
        .await
        .map_err(AppError::GitHub)?;

    // Deduplicate reviews, keeps latest verdict per reviewer.
    // IndexMap preserves insertion order so display is stable across updates.
    let mut reviewers_by_login: IndexMap<String, ReviewState> = IndexMap::new();

    for review in raw_reviewers {
        let login = review.user.map(|u| u.login).unwrap_or_default();
        if login.is_empty() {
            continue;
        }
        let state = map_review_state(review.state);
        reviewers_by_login.insert(login, state);
    }

    // Requested reviewer with no submitted review or comments are pending.
    for user in pr.requested_reviewers {
        reviewers_by_login
            .entry(user.login)
            .or_insert(ReviewState::Pending);
    }

    let reviews = resolve_discord_tags(reviewers_by_login, user_store).await;

    let total_comments = pr.comments + pr.review_comments;

    Ok(PrMessageData {
        status_emoji: pr_ref.status_emoji(),
        number: pr_ref.number,
        title: pr_ref.title.clone(),
        author: pr_ref.user.login.clone(),
        repo: full_name.to_owned(),
        head: pr_ref.head.branch.clone(),
        base: pr_ref.base.branch.clone(),
        url: pr_ref.html_url.clone(),
        additions: pr.additions,
        deletions: pr.deletions,
        files: pr.changed_files,
        commits: pr.commits,
        comments: total_comments,
        reviews,
        checks: vec![],
    })
}

/// Map octocrab's review db to our domain type.
const fn map_review_state(state: Option<octocrab::models::pulls::ReviewState>) -> ReviewState {
    match state {
        Some(octocrab::models::pulls::ReviewState::Approved) => ReviewState::Approved,
        Some(octocrab::models::pulls::ReviewState::ChangesRequested) => {
            ReviewState::ChangesRequested
        }
        Some(octocrab::models::pulls::ReviewState::Dismissed) => ReviewState::Dismissed,
        _ => ReviewState::Commented,
    }
}

/// Look up Discord tags for each reviewer and attach them if found.
async fn resolve_discord_tags(
    reviewers: IndexMap<String, ReviewState>,
    user_store: &dyn UserLinkStore,
) -> Vec<ReviewSummary> {
    let mut result = Vec::with_capacity(reviewers.len());

    for (github_login, state) in reviewers {
        let discord_tag = user_store
            .get_by_github(&github_login)
            .await
            .ok()
            .flatten()
            .map(|link| format!("<@{}>", link.discord_id));

        result.push(ReviewSummary {
            github_login,
            discord_tag,
            state,
        });
    }

    result
}
