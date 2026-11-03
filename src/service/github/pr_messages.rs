//! Full pull request data required to construct the `Discord` messages seen by users.
//!
//! Responsible for constructing, loading, and sending updates made to PRs triggered
//! by `GitHub` webhook events.

use crate::discord::messaging::messages::update_pull_request_message;
use crate::error::AppError;
use crate::github::api;
use crate::github::api::client::GitHubClient;
use crate::github::webhook::events::models::{GitHubUserInfo, PullRequestExt, PullRequestInfo};
use crate::models::pr_message::{PrMessage, PrStore};
use crate::models::subscription::SubscriptionStore;
use crate::models::user_link::UserStore;
use indexmap::IndexMap;
use octocrab::models::pulls::{Review, ReviewState};
use octocrab::Octocrab;
use serenity::all::{ChannelId, Http};
use std::collections::HashMap;

pub struct PrMessageData {
	pub status_emoji: &'static str,
	pub number: u64,
	pub title: String,
	pub author: String,
	pub repository: String,
	pub head: String,
	pub base: String,
	pub pr_url: String,
	pub html_url: String,
	pub additions: u64,
	pub deletions: u64,
	pub files: u64,
	pub commits: u64,
	pub comments: u64,
	pub reviews: Vec<ReviewSummary>,
	pub requested_reviewers: Vec<String>,
	pub checks: Vec<CheckSummary>,
}

pub struct ReviewSummary {
	pub github_login: String,
	pub discord_tag: Option<String>,
	pub state: ReviewState,
}

pub struct CheckSummary {
	pub name: String,
	pub conclusion: CheckStatus,
}

struct ReviewerState {
	github_id: Option<u64>,
	state: ReviewState,
}

#[allow(unused)]
pub enum CheckStatus {
	Success,
	Failure,
	Pending,
}

impl PrMessageData {
	pub fn new(
		owner: &str,
		project: &str,
		pr: &PullRequestInfo,
		reviewers: Vec<ReviewSummary>,
	) -> Self {
		Self {
			status_emoji: pr.status_emoji(),
			number: pr.number,
			title: pr.title.clone(),
			author: pr.user.login.clone(),
			repository: format!("{owner}/{project}"),
			head: pr.head.branch.clone(),
			base: pr.base.branch.clone(),
			pr_url: pr.html_url.clone(),
			html_url: format!("https://github.com/{owner}/{project}"),
			additions: pr.additions,
			deletions: pr.deletions,
			files: pr.changed_files,
			commits: pr.commits,
			comments: pr.comments.saturating_add(pr.review_comments),
			reviews: reviewers,
			requested_reviewers: pr
				.requested_reviewers
				.iter()
				.map(|reviewer| reviewer.login.clone())
				.collect(),
			checks: vec![],
		}
	}
}

pub async fn load_pr_message_data(
	github: &GitHubClient,
	sub_store: &dyn SubscriptionStore,
	user_store: &dyn UserStore,
	owner: &str,
	project: &str,
	pr_number: u64,
) -> Result<Option<PrMessageData>, AppError> {
	let Some(gh_install_client) =
		installation_client_for_repo(sub_store, github, owner, project).await?
	else {
		return Ok(None);
	};

	let pr = PullRequestInfo::from(
		api::pull_requests::fetch(&gh_install_client, owner, project, pr_number).await?,
	);
	let submitted_reviews =
		api::pull_requests::fetch_reviews(&gh_install_client, owner, project, pr_number).await?;

	let reviewer_states = merge_review_states(&submitted_reviews, &pr.requested_reviewers);
	let reviewers = enrich_reviewers(user_store, reviewer_states).await;

	Ok(Some(PrMessageData::new(owner, project, &pr, reviewers)))
}

async fn installation_client_for_repo(
	sub_store: &dyn SubscriptionStore,
	github: &GitHubClient,
	owner: &str,
	project: &str,
) -> Result<Option<Octocrab>, AppError> {
	let Some(id) = sub_store
		.fetch_installation_id_by_owner_project(owner, project)
		.await?
	else {
		return Ok(None);
	};
	Ok(Some(github.scoped_to_installation(id)?))
}

/// Merge submitted reviews with requested reviewers.
fn merge_review_states(
	submitted_reviews: &[Review],
	requested_reviewers: &[GitHubUserInfo],
) -> IndexMap<String, ReviewerState> {
	let mut states = IndexMap::new();

	for review in submitted_reviews {
		if let Some(user) = &review.user {
			let login = user.login.clone();
			if !login.is_empty() {
				let state = review.state.unwrap_or(ReviewState::Commented);
				states.insert(
					login,
					ReviewerState {
						github_id: Some(user.id.0),
						state,
					},
				);
			}
		}
	}

	for reviewer in requested_reviewers {
		states
			.entry(reviewer.login.clone())
			.or_insert(ReviewerState {
				github_id: reviewer.id,
				state: ReviewState::Pending,
			});
	}

	states
}

/// Displays a user's `Discord` tag when a link to their `GitHub` login is found.
async fn enrich_reviewers(
	user_store: &dyn UserStore,
	reviewers: IndexMap<String, ReviewerState>,
) -> Vec<ReviewSummary> {
	let logins: Vec<String> = reviewers.keys().cloned().collect();
	let github_ids: Vec<i64> = reviewers
		.values()
		.filter_map(|r| r.github_id)
		.map(u64::cast_signed)
		.collect();

	let by_id = user_store
		.fetch_by_github_user_ids(&github_ids)
		.await
		.unwrap_or_default();
	let by_login = user_store
		.fetch_by_github_logins(&logins)
		.await
		.unwrap_or_default();

	reviewers
		.into_iter()
		.map(|(github_login, ReviewerState { github_id, state })| {
			let discord_id = resolve_discord_id(github_id, &github_login, &by_id, &by_login);
			ReviewSummary {
				github_login,
				discord_tag: discord_id.map(|id| format!("<@{id}>")),
				state,
			}
		})
		.collect()
}

/// ID match first (rename-proof, wins over any login collision), then login
/// match (legacy rows), then none.
fn resolve_discord_id(
	github_id: Option<u64>,
	login: &str,
	by_id: &HashMap<i64, u64>,
	by_login: &HashMap<String, u64>,
) -> Option<u64> {
	github_id
		.map(u64::cast_signed)
		.and_then(|id| by_id.get(&id).copied())
		.or_else(|| by_login.get(login).copied())
}

/// Updates stored PR messages across all subscribed channels with new webhook activity in place.
///
/// A failure updating one channel aborts the batch and propagates; callers
/// that need best-effort delivery should handle that at their level.
/// Returns the list of message records that were targeted.
pub async fn update_all_pr_messages(
	pr_store: &dyn PrStore,
	http: &Http,
	owner: &str,
	project: &str,
	pr_number: u64,
	message_data: &PrMessageData,
) -> Result<Vec<PrMessage>, AppError> {
	let pr_messages = pr_store
		.fetch_all_by_repository_and_pr(owner, project, pr_number)
		.await?;
	for msg in &pr_messages {
		update_pull_request_message(
			http,
			ChannelId::from(msg.channel_id),
			msg.message_id,
			message_data,
		)
		.await?;
	}

	// Syncs requested reviewer baseline so accurate diffs can be created later.
	pr_store
		.update_requested_reviewers(owner, project, pr_number, &message_data.requested_reviewers)
		.await?;

	Ok(pr_messages)
}
