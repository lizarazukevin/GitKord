//! PR message formatting.
//!
//! Pure functions only — no I/O, no async. Takes domain data and returns
//! a Discord-formatted string. Swap this out when moving to embeds.

use crate::github::models::{CheckStatus, PrMessageData, ReviewState};

/// Format the full PR message string from assembled PR data.
pub fn format_pr_message(data: &PrMessageData) -> String {
    let repo_base_url = data.url.split("/pull").next().unwrap_or(&data.url);

    let branches = format!(
        "[`{head}`](<{url}/tree/{head}>) → [`{base}`](<{url}/tree/{base}>)",
        head = data.head,
        base = data.base,
        url = repo_base_url,
    );

    let checks_section = format_checks(data);
    let reviewers_section = format_reviewers(data);

    format!(
        "## {status_emoji} PR #{number} — {title}\n\
         > ↳ 👤 **{author}**  **·**  🌿 {branches}  **·**  📦 [{repo}](<{url}>)\n\n\
         {bar}\n\n\
         {stats}\n\
         {checks_section}\
         {reviewers_section}\n\
         -# *Last updated: {timestamp}*",
        status_emoji = data.status_emoji,
        number = data.number,
        title = data.title,
        author = data.author,
        branches = branches,
        repo = data.repo,
        url = data.url,
        bar = format_diff_bar(data),
        stats = format_stats(data),
        checks_section = checks_section,
        reviewers_section = reviewers_section,
        timestamp = timestamp(),
    )
}

const BAR_WIDTH: u64 = 10;
const LINES_PER_BLOCK: u64 = 100;
const PROPORTIONAL_THRESHOLD: u64 = BAR_WIDTH * LINES_PER_BLOCK;

// Diff bar — additions on the left, deletions on the right, 10 blocks total.
fn format_diff_bar(data: &PrMessageData) -> String {
    let (add_filled, del_filled) = split_bar(data.additions, data.deletions);

    format!(
        "+{}  {}{}  -{}",
        data.additions,
        "🟩 ".repeat(usize::try_from(add_filled).unwrap_or(usize::MIN)),
        "🟥 ".repeat(usize::try_from(del_filled).unwrap_or(usize::MIN)),
        data.deletions,
    )
}

// Decide how many of the BAR_WIDTH blocks go to additions vs. deletions.
fn split_bar(additions: u64, deletions: u64) -> (u64, u64) {
    let total = additions.saturating_add(deletions);

    if total == 0 {
        return (0, 0);
    }

    if total < PROPORTIONAL_THRESHOLD {
        let add_blocks = additions.div_ceil(LINES_PER_BLOCK).min(BAR_WIDTH);
        let del_blocks = deletions
            .div_ceil(LINES_PER_BLOCK)
            .min(BAR_WIDTH - add_blocks);
        (add_blocks, del_blocks)
    } else {
        let add_blocks = additions
            .saturating_mul(BAR_WIDTH)
            .checked_div(total)
            .unwrap_or(0)
            .min(BAR_WIDTH);
        let del_blocks = BAR_WIDTH - add_blocks;
        (add_blocks, del_blocks)
    }
}

// File, commit and comment counts.
fn format_stats(data: &PrMessageData) -> String {
    format!(
        "📁 *{} files*  **·**  ✨ *{} commits*  **·**  💬 *{} comments*",
        data.files, data.commits, data.comments,
    )
}

// CI check pipeline — empty string when no checks are present.
fn format_checks(data: &PrMessageData) -> String {
    if data.checks.is_empty() {
        return String::new();
    }

    let checks = data
        .checks
        .iter()
        .map(|c| {
            let emoji = match c.conclusion {
                CheckStatus::Success => "🟢",
                CheckStatus::Failure => "🔴",
                CheckStatus::Pending => "⚪",
            };
            format!("{emoji} {}", c.name)
        })
        .collect::<Vec<_>>()
        .join("  →  ");

    format!("\n### Checks\n{checks}\n")
}

// Reviewer list grouped by verdict. Falls back to a hint when none are assigned.
fn format_reviewers(data: &PrMessageData) -> String {
    if data.reviews.is_empty() {
        return "\n### Reviewers\n*No reviewers assigned (use `/assign` to request a review)*\n"
            .to_owned();
    }

    // Group by verdict emoji, preserving the order verdicts first appear.
    let mut grouped: indexmap::IndexMap<&str, Vec<String>> = indexmap::IndexMap::new();

    for review in &data.reviews {
        let display = review.discord_tag.as_ref().map_or_else(
            || {
                format!(
                    "[`{}`](<https://github.com/{}>)",
                    review.github_login, review.github_login
                )
            },
            std::clone::Clone::clone,
        );

        let key = match review.state {
            ReviewState::Approved => "✅",
            ReviewState::ChangesRequested => "🛑",
            ReviewState::Commented | ReviewState::Dismissed => "💬",
            ReviewState::Pending => "🟡",
        };

        grouped.entry(key).or_default().push(display);
    }

    let body = grouped
        .iter()
        .map(|(emoji, names)| format!("{}  **|**  {}", emoji, names.join("  ·  ")))
        .collect::<Vec<_>>()
        .join("\n");

    format!("\n### Reviewers\n{body}\n")
}

// Short UTC timestamp used in the footer and audit entries.
pub fn timestamp() -> String {
    chrono::Utc::now()
        .format("%d %b %Y at %H:%M UTC")
        .to_string()
}
