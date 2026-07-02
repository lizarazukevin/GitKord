//! Audit thread entries.
//!
//! Every significant PR event gets a timestamped entry appended to the
//! audit thread. These are write-only — we never read them back, they
//! exist purely for human traceability.

use serenity::all::Http;

use crate::discord::messages::transport::post_to_thread;
use crate::error::Result;
use crate::github::payloads::PullRequestReviewPayload;

/// Append a PR lifecycle change to the audit thread.
///
/// Used for opened, closed, reopened, synchronize — anything that changes
/// the PR's overall db rather than reviewer verdicts.
pub async fn post_pr_update(
    http: &Http,
    thread_id: u64,
    action: &str,
    merged: Option<bool>,
) -> Result<()> {
    let (emoji, verb) = match (action, merged) {
        ("closed", Some(true)) => ("🟣", "merged"),
        ("closed", _) => ("🔴", "closed"),
        ("reopened", _) => ("🟢", "reopened"),
        _ => ("⚪", action),
    };

    let content = format!("{emoji} this review is {verb}");
    post_to_thread(http, thread_id, &content).await
}

/// Append a review verdict to the audit thread.
///
/// Called when a `pull_request_review` event is submitted or dismissed.
pub async fn post_review(
    http: &Http,
    thread_id: u64,
    payload: &PullRequestReviewPayload,
) -> Result<()> {
    let review = &payload.review;

    let verb = match review.state.to_lowercase().as_str() {
        "approved" => "approved this review",
        "changes_requested" => "requested changes",
        "commented" => "published comments",
        _ => "submitted a review",
    };

    let content = format!(
        "{emoji} **{reviewer}** {verb}",
        emoji = review.verdict_emoji(),
        reviewer = review.user.login
    );

    post_to_thread(http, thread_id, &content).await
}

/// Append a reviewer assignment change to the audit thread.
///
/// Called from the assign/unassign slash command handlers after a
/// successful GitHub API call.
pub async fn post_reviewer_change(
    http: &Http,
    thread_id: u64,
    actor: &str,
    reviewer: &str,
    assigned: bool,
) -> Result<()> {
    let action = if assigned {
        format!("👥 **{actor}** requested review from **{reviewer}**")
    } else {
        format!("👤 **{actor}** removed review request from **{reviewer}**")
    };

    post_to_thread(http, thread_id, &action).await
}

/// Append a commit push notification to the audit thread.
///
/// Called when a `synchronize` event fires on a PR — someone pushed
/// new commits to the branch.
pub async fn post_commit_push(http: &Http, thread_id: u64, pusher: &str, sha: &str) -> Result<()> {
    let short_sha = &sha[..7.min(sha.len())];
    let content = format!("📬 **{pusher}** pushed commit `{short_sha}`");
    post_to_thread(http, thread_id, &content).await
}
