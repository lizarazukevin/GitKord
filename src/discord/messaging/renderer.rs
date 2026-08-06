//! PR message formatting.

use crate::service::github::pr_messages::{CheckStatus, CheckSummary, PrMessageData};
use indexmap::IndexMap;
use octocrab::models::pulls::ReviewState;

const DIFF_BAR_WIDTH: u64 = 10;
const DIFF_BAR_FULL_AT: u64 = 800;
const GROWTH_EXPONENT: f64 = 0.4;

/// Assemble the full PR status message.
pub(super) fn format_pr_message(data: &PrMessageData, timestamp: &str) -> String {
	let repo_base_url = data.url.split("/pull").next().unwrap_or(&data.url);

	let branches = format!(
		"[`{head}`](<{url}/tree/{head}>) → [`{base}`](<{url}/tree/{base}>)",
		head = data.head,
		base = data.base,
		url = repo_base_url,
	);

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
		repo = data.repository,
		url = data.url,
		bar = format_diff_bar(data.additions, data.deletions),
		stats = format_pr_stats(data.files, data.commits, data.comments),
		checks_section = format_checks(&data.checks),
		reviewers_section = format_reviewers(data),
		timestamp = timestamp,
	)
}

/// Build the colored diff stat bar string.
///
/// Displays `+N 🟩…🟥… -M` where the number of green/red blocks is
/// proportional to additions vs deletions, clamped to `BAR_WIDTH`.
fn format_diff_bar(additions: u64, deletions: u64) -> String {
	let (add_filled, del_filled) = split_diff_blocks(additions, deletions);

	format!(
		"+{}  {}{}  -{}",
		additions,
		"🟩 ".repeat(usize::try_from(add_filled).unwrap_or(usize::MIN)),
		"🟥 ".repeat(usize::try_from(del_filled).unwrap_or(usize::MIN)),
		deletions,
	)
}

/// Decide how many of the lit blocks represent additions vs deletions.
fn split_diff_blocks(additions: u64, deletions: u64) -> (u64, u64) {
	let total_lines = additions.saturating_add(deletions);

	if total_lines == 0 {
		return (0, 0);
	}

	let lit_blocks = blocks_to_light(total_lines);

	let add_ratio = to_f64(additions) / to_f64(total_lines);
	let add_blocks = to_u64(to_f64(lit_blocks) * add_ratio);

	let add_blocks = add_blocks.min(lit_blocks);
	let del_blocks = lit_blocks.saturating_sub(add_blocks);

	let add_blocks = if additions > 0 { add_blocks.max(1) } else { 0 };
	let del_blocks = if deletions > 0 { del_blocks.max(1) } else { 0 };

	(add_blocks, del_blocks)
}

/// Number of bar blocks that should be lit for a given total line count.
///
/// Uses a sub‑linear growth curve so the bar is useful for both tiny and
/// very large PRs.
fn blocks_to_light(total_lines: u64) -> u64 {
	let ratio = (to_f64(total_lines) / to_f64(DIFF_BAR_FULL_AT)).min(1.0);

	to_u64(to_f64(DIFF_BAR_WIDTH) * ratio.powf(GROWTH_EXPONENT))
}

#[expect(
	clippy::cast_precision_loss,
	clippy::as_conversions,
	reason = "Visualization calculations require converting bounded counts to floating point"
)]
const fn to_f64(value: u64) -> f64 {
	value as f64
}

#[expect(
	clippy::cast_possible_truncation,
	clippy::cast_sign_loss,
	clippy::as_conversions,
	reason = "Visualization values are rounded and bounded by display dimensions"
)]
const fn to_u64(value: f64) -> u64 {
	value.round() as u64
}

/// Compact line showing files, commits, and comments.
fn format_pr_stats(files: u64, commits: u64, comments: u64) -> String {
	format!("📁 *{files} files*  **·**  ✨ *{commits} commits*  **·**  💬 *{comments} comments*")
}

/// Render the CI check pipeline, or an empty string if there are none.
fn format_checks(checks: &[CheckSummary]) -> String {
	if checks.is_empty() {
		return String::new();
	}

	let checks = checks
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

/// Reviewer list grouped by verdict, or a hint when none are assigned.
fn format_reviewers(data: &PrMessageData) -> String {
	if data.reviews.is_empty() {
		return "\n### Reviewers\n*No reviewers assigned (use `/assign` to request a review)*\n"
			.to_owned();
	}

	let mut grouped: IndexMap<&str, Vec<String>> = IndexMap::new();

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
			_ => "🟡",
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
