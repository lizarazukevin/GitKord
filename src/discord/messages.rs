//! Discord message formatting and creation for PR events.
//!
//! Public functions map to webhook event types and produces
//! a formatted Discord message.

use serenity::all::{ChannelId, CreateMessage, CreateThread, EditMessage, Http, MessageId};
use tracing::info;

use crate::error::AppError;
use crate::error::Result;
use crate::github::models::{CheckStatus, PrMessageData, ReviewState};
use crate::github::payloads::{PullRequestPayload, PullRequestReviewPayload};

/// Post the main PR message to a channel and create an audit thread.
///
/// Returns `(message_id, thread_id)` — both must persist so future events
/// can edit the message and append to the thread.
pub async fn post_pull_request(
    http: &Http,
    channel_id: ChannelId,
    payload: &PullRequestPayload,
    message_data: &PrMessageData,
) -> Result<(u64, u64)> {
    let pr = &payload.pull_request;

    let message = channel_id
        .send_message(
            http,
            CreateMessage::new().content(format_pr_message(message_data)),
        )
        .await
        .map_err(|e| AppError::Discord(Box::new(e)))?;

    let thread_name = format!(
        "PR #{} — {} audit log",
        pr.number, payload.repository.full_name
    );
    let thread = channel_id
        .create_thread_from_message(
            http,
            message.id,
            CreateThread::new(thread_name)
                .auto_archive_duration(serenity::all::AutoArchiveDuration::OneWeek),
        )
        .await
        .map_err(|e| AppError::Discord(Box::new(e)))?;

    info!(
        channel = %channel_id,
        message = %message.id,
        thread = %thread.id,
        pr      = pr.number,
        "posted PR message and created audit thread"
    );

    post_to_thread(
        http,
        thread.id.get(),
        &format!("🟢 **{}** opened this PR", pr.user.login),
    )
    .await?;

    Ok((message.id.get(), thread.id.get()))
}

/// Edit the main PR message in place and append an entry to the audit thread.
///
/// Used for `synchronize`, `closed`, `reopened` actions.
pub async fn update_pull_request(
    http: &Http,
    channel_id: ChannelId,
    message_id: u64,
    thread_id: u64,
    payload: &PullRequestPayload,
    message_data: &PrMessageData,
) -> Result<()> {
    channel_id
        .edit_message(
            http,
            MessageId::new(message_id),
            EditMessage::new().content(format_pr_message(message_data)),
        )
        .await
        .map_err(|e| AppError::Discord(Box::new(e)))?;

    let content = format!(
        "🔄 **{}** — PR #{} `{}`",
        timestamp(),
        payload.pull_request.number,
        payload.action,
    );
    post_to_thread(http, thread_id, &content).await?;

    info!(
        channel = %channel_id,
        message = message_id,
        thread = thread_id,
        pr      = payload.pull_request.number,
        action  = %payload.action,
        "updated PR message in Discord"
    );

    Ok(())
}

/// Post a review event to the PR audit thread.
pub async fn post_review(
    http: &Http,
    thread_id: u64,
    payload: &PullRequestReviewPayload,
) -> Result<()> {
    let review = &payload.review;
    let pr = &payload.pull_request;

    let verb = match review.state.to_lowercase().as_str() {
        "approved" => "approved",
        "changes_requested" => "requested changes on",
        _ => "commented on",
    };

    let content = format!(
        "{emoji} **{reviewer}** {verb} PR #{number}",
        emoji = review.verdict_emoji(),
        reviewer = review.user.login,
        number = pr.number,
    );
    post_to_thread(http, thread_id, &content).await?;

    Ok(())
}

/// Post an audit entry to a thread. Used by commands (assign/unassign) and
/// event handlers alike so there is one place to change thread posting behavior.
pub async fn post_to_thread(http: &Http, thread_id: u64, content: &str) -> Result<()> {
    ChannelId::new(thread_id)
        .send_message(http, CreateMessage::new().content(content))
        .await
        .map_err(|e| AppError::Discord(Box::new(e)))?;
    Ok(())
}

/// Format a PR payload into the main channel message.
///
/// Plain string for now — will move to embeds once the core flow is
/// proven end-to-end.
fn format_pr_message(data: &PrMessageData) -> String {
    let status_emoji = data.status_emoji;

    let repo_base_url = data.url.split("/pull").next().unwrap_or(&data.url);

    let pr_url = format!("<{}>", data.url);

    let branches = format!(
        "[`{head}`](<{url}/tree/{head}>) → [`{base}`](<{url}/tree/{base}>)",
        head = data.head,
        base = data.base,
        url = repo_base_url,
    );

    let total = (data.additions + data.deletions).max(1);
    let add_filled = usize::try_from((data.additions * 10 / total).min(10)).unwrap_or(10);
    let del_filled = 10 - add_filled;
    let bar = format!(
        "+{}  {}{}  -{}",
        data.additions,
        "🟩 ".repeat(add_filled),
        "🟥 ".repeat(del_filled),
        data.deletions,
    );

    let stats = format!(
        "📁 *{} files*  **·**  ✨ *{} commits*  **·**  💬 *{} comments*",
        data.files, data.commits, data.comments,
    );

    let checks_section = if data.checks.is_empty() {
        String::new()
    } else {
        let checks = data
            .checks
            .iter()
            .map(|c| {
                let emoji = match c.conclusion {
                    CheckStatus::Success => "🟢",
                    CheckStatus::Failure => "🔴",
                    CheckStatus::Pending => "⚪",
                };
                format!("{emoji} **{}**", c.name)
            })
            .collect::<Vec<_>>()
            .join("  →  ");
        format!("\n### Checks\n{checks}\n")
    };

    let reviewers_section = if data.reviews.is_empty() {
        "\n### Reviewers:\n*No reviewers assigned (use `/assign` to request a review)*\n".to_owned()
    } else {
        let mut grouped: std::collections::BTreeMap<&str, Vec<String>> =
            std::collections::BTreeMap::new();

        for r in &data.reviews {
            let entry = format!(
                "{}[`{}`](<https://github.com/{}>)",
                r.discord_tag
                    .as_deref()
                    .map(|d| format!("@{d}  ·  "))
                    .unwrap_or_default(),
                r.github_login,
                r.github_login,
            );
            let key = match r.state {
                ReviewState::Approved => "✅",
                ReviewState::ChangesRequested => "🛑",
                ReviewState::Commented => "💬",
                ReviewState::Pending => "🟡",
            };
            grouped.entry(key).or_default().push(entry);
        }

        let body = grouped
            .iter()
            .map(|(emoji, names)| format!("{}  **|**  {}", emoji, names.join("  ·  ")))
            .collect::<Vec<_>>()
            .join("\n");

        format!("\n### Reviewers\n{body}\n")
    };

    format!(
        "## {status_emoji} PR #{number} — {title}\n\
     > ↳ 👤 **{author}**  **·**  🌿 {branches}  **·**  📦 [{repo}]({url})\n\n\
     {bar}\n\n\
     {stats}\n\n\
     {checks_section}
     {reviewers_section}
     \n-# *Last updated: {timestamp}*",
        number = data.number,
        title = data.title,
        author = data.author,
        branches = branches,
        repo = data.repo,
        url = pr_url,
        bar = bar,
        stats = stats,
        checks_section = checks_section,
        reviewers_section = reviewers_section,
        timestamp = timestamp(),
    )
}

/// Returns a short UTC timestamp string for audit entries.
fn timestamp() -> String {
    chrono::Utc::now()
        .format("%d %b %Y at %H:%M UTC")
        .to_string()
}
