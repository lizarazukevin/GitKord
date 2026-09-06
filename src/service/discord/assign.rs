//! Service layer for `/assign` and `/unassign` commands.

use crate::discord::commands::args::ReviewerInput;
use crate::discord::messaging::audit::post_to_thread;
use crate::error::{format_error, AppError};
use crate::github;
use crate::github::api::client::GitHubClient;
use crate::github::api::pull_requests::split_repo;
use crate::models::pr_message::PrStore;
use crate::models::subscription::SubscriptionStore;
use crate::models::user_link::UserStore;
use serenity::http::Http;
use std::sync::Arc;
use tracing::{error, info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignAction {
	Assign,
	Unassign,
}

pub struct AssignRequest {
	pub reviewer: ReviewerInput,
	pub action: AssignAction,
	pub actor: String,
	pub repo: Option<String>,
	pub pr: Option<u64>,
	pub channel_id: u64,
}

pub struct AssignService {
	pr_store: Arc<dyn PrStore>,
	sub_store: Arc<dyn SubscriptionStore>,
	user_store: Arc<dyn UserStore>,
	github: Arc<GitHubClient>,
}

impl AssignService {
	pub fn new(
		pr_store: Arc<dyn PrStore>,
		sub_store: Arc<dyn SubscriptionStore>,
		user_store: Arc<dyn UserStore>,
		github: Arc<GitHubClient>,
	) -> Self {
		Self {
			pr_store,
			sub_store,
			user_store,
			github,
		}
	}

	/// Resolve the PR context and reviewer, then (un)assign on `GitHub`
	/// and send audit message on the thread.
	pub async fn handle(&self, req: AssignRequest, http: &Http) -> Result<String, AppError> {
		let (repo, pr) = self
			.resolve_pr_context(req.channel_id, req.repo, req.pr)
			.await?;

		let (owner, project) = split_repo(&repo)?;

		let reviewer_login = self.resolve_reviewer_login(&req.reviewer).await?;

		if github::api::users::verify(&self.github, &reviewer_login)
			.await?
			.is_none()
		{
			return Err(AppError::message(format_error(
				"GitHub user not found",
				None,
			)));
		}

		let Some(installation_id) = self
			.sub_store
			.fetch_installation_id_by_owner_project(owner, project)
			.await?
		else {
			return Err(AppError::message(format_error(
				"Repository is not subscribed",
				Some("Run `/subscribe` in a channel first."),
			)));
		};
		let gh_install_client = self.github.scoped_to_installation(installation_id)?;

		let requested_revs = github::api::pull_requests::fetch_requested_reviewers(
			&gh_install_client,
			owner,
			project,
			pr,
		)
		.await?;

		let is_requested = requested_revs.contains(&reviewer_login);

		match req.action {
			AssignAction::Assign => {
				if is_requested {
					return Err(AppError::message(format_error(
						"Review has already been requested by that user",
						None,
					)));
				}
				github::api::pull_requests::assign_reviewer(
					&gh_install_client,
					&repo,
					pr,
					&reviewer_login,
				)
				.await
				.map_err(|_| {
					AppError::message(format_error(
						"Could not request a review from that user",
						Some("Verify the reviewer has write/pull access and is not the PR author."),
					))
				})?;
			}
			AssignAction::Unassign => {
				if !is_requested {
					return Err(AppError::message(format_error(
						"That user is not currently a requested reviewer",
						Some("They may have already submitted a review or were never requested."),
					)));
				}
				github::api::pull_requests::unassign_reviewer(
					&gh_install_client,
					&repo,
					pr,
					&reviewer_login,
				)
				.await
				.map_err(|_| {
					AppError::message(format_error(
						"Could not remove the reviewer",
						Some(
							"Verify the user was actually requested and the PR number is correct.",
						),
					))
				})?;
			}
		}

		info!(
			pr,
			reviewer = %reviewer_login,
			repo = %repo,
			"reviewer {}",
			if req.action == AssignAction::Assign { "assigned" } else { "unassigned" }
		);

		self.broadcast_audit(http, &repo, pr, &req.actor, &reviewer_login, req.action)
			.await;

		Ok(match req.action {
			AssignAction::Assign => {
				format!("Requested review from **{reviewer_login}** on PR #**{pr}** in **{repo}**")
			}
			AssignAction::Unassign => format!(
				"Removed review request from **{reviewer_login}** on PR #**{pr}** in **{repo}**"
			),
		})
	}

	/// Determine the target `(repository, pr)` from explicit args, or fall
	/// back to the PR thread the command was run in.
	async fn resolve_pr_context(
		&self,
		channel_id: u64,
		repo: Option<String>,
		pr: Option<u64>,
	) -> Result<(String, u64), AppError> {
		match (&repo, &pr) {
			(Some(repo), Some(pr)) => return Ok((repo.clone(), *pr)),
			(None, None) => {}
			_ => {
				return Err(AppError::message(format_error(
					"Missing fields",
					Some("Provide both `repository` and `pr`, or neither (inside a PR thread)."),
				)))
			}
		}

		let pr_msg = self
			.pr_store
			.fetch_by_thread_id(channel_id)
			.await?
			.ok_or_else(|| {
				AppError::message(format_error(
					"Missing fields",
					Some("Run this inside a PR thread, or provide both `repository` and `pr`."),
				))
			})?;

		Ok((pr_msg.repository, pr_msg.pr))
	}

	/// Resolve a reviewer input (`GitHub` login or `Discord` ID) to a `GitHub` login.
	async fn resolve_reviewer_login(&self, reviewer: &ReviewerInput) -> Result<String, AppError> {
		match reviewer {
			ReviewerInput::GitHubLogin(login) => Ok(login.clone()),
			ReviewerInput::DiscordMention(discord_id) => {
				let link = self
					.user_store
					.fetch_by_discord_id(*discord_id)
					.await?
					.ok_or_else(|| {
						AppError::message(format_error(
							"Discord user has not linked their GitHub account",
							Some("Ask them to run `/link` first."),
						))
					})?;
				Ok(link.github_login)
			}
		}
	}

	/// Sends an assign audit message to every saved PR message's thread.
	async fn broadcast_audit(
		&self,
		http: &Http,
		repo: &str,
		pr: u64,
		actor: &str,
		reviewer_login: &str,
		action: AssignAction,
	) {
		let pr_messages = match self.pr_store.fetch_all_by_repo_and_pr(repo, pr).await {
			Ok(messages) => messages,
			Err(e) => {
				error!(error = %e, repo, pr, "failed to fetch PR messages for audit");
				return;
			}
		};

		if pr_messages.is_empty() {
			warn!(repo, pr, "no PR message records found for audit post");
			return;
		}

		let msg = match action {
			AssignAction::Assign => {
				format!("👥 **{actor}** requested review from **{reviewer_login}**")
			}
			AssignAction::Unassign => {
				format!("👤 **{actor}** removed review request from **{reviewer_login}**")
			}
		};

		for message in pr_messages {
			if let Err(e) = post_to_thread(http, message.thread_id, &msg).await {
				error!(error = %e, thread_id = message.thread_id, "failed to post audit message");
			}
		}
	}
}
