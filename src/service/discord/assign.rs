//! Service layer for `/assign` and `/unassign` commands.

use crate::discord::commands::args::ReviewerInput;
use crate::discord::messaging::audit::broadcast_to_threads;
use crate::error::{format_error, AppError};
use crate::github;
use crate::github::api::client::GitHubClient;
use crate::github::api::pull_requests::{join_repo, split_repo};
use crate::models::pr_message::PrStore;
use crate::models::subscription::SubscriptionStore;
use crate::models::user_link::UserStore;
use octocrab::Octocrab;
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
	pub actor_discord_id: u64,
	pub repo: Option<String>,
	pub pr: Option<u64>,
	pub channel_id: u64,
}

/// Resolved state for an assign/unassign, shared by the audit and response paths.
struct AssignContext {
	owner: String,
	project: String,
	pr: u64,
	actor: String,
	reviewer_login: String,
	reviewer_display: String,
	action: AssignAction,
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
		let (owner, project, pr) = self
			.resolve_pr_context(req.channel_id, req.repo, req.pr)
			.await?;
		let repo = join_repo(&owner, &project);
		let reviewer_login = self.resolve_reviewer_login(&req.reviewer).await?;

		self.ensure_reviewer_exists(&reviewer_login).await?;
		let gh_install_client = self.installation_client(&owner, &project).await?;

		let requested_revs = github::api::pull_requests::fetch_requested_reviewers(
			&gh_install_client,
			&owner,
			&project,
			pr,
		)
		.await?;
		let is_requested = requested_revs.contains(&reviewer_login);

		// The actor is always kept in `GitHub`-login form (plain text, never a
		// mention) so the audit entry cannot ping the person who ran the command.
		let actor = self
			.resolve_discord_user_login(req.actor_discord_id)
			.await?;
		let reviewer_display = self
			.resolve_reviewer_display(&reviewer_login, req.actor_discord_id)
			.await;

		let ctx = AssignContext {
			owner,
			project,
			pr,
			actor,
			reviewer_login,
			reviewer_display,
			action: req.action,
		};

		apply_review_change(&gh_install_client, &ctx, is_requested).await?;

		info!(
			pr = ctx.pr,
			reviewer = %ctx.reviewer_login,
			repo = %repo,
			"reviewer {}",
			if ctx.action == AssignAction::Assign { "assigned" } else { "unassigned" }
		);

		self.broadcast_audit(http, &ctx).await;

		Ok(match ctx.action {
			AssignAction::Assign => format!(
				"Requested a review from **{}** on PR #**{}** in **{}**",
				ctx.reviewer_login, ctx.pr, repo
			),
			AssignAction::Unassign => format!(
				"Removed review request from **{}** on PR #**{}** in **{}**",
				ctx.reviewer_login, ctx.pr, repo
			),
		})
	}

	/// Verify a `GitHub` login actually exists before requesting a review from it.
	async fn ensure_reviewer_exists(&self, reviewer_login: &str) -> Result<(), AppError> {
		if github::api::users::verify(&self.github, reviewer_login)
			.await?
			.is_none()
		{
			return Err(AppError::message(format_error(
				"GitHub user not found",
				None,
			)));
		}
		Ok(())
	}

	/// Build an installation-scoped `GitHub` client for the subscribed repository.
	async fn installation_client(&self, owner: &str, project: &str) -> Result<Octocrab, AppError> {
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
		self.github.scoped_to_installation(installation_id)
	}

	/// Determine the target `(owner, project, pr)` from explicit args, or fall
	/// back to the PR thread the command was run in.
	async fn resolve_pr_context(
		&self,
		channel_id: u64,
		repo: Option<String>,
		pr: Option<u64>,
	) -> Result<(String, String, u64), AppError> {
		if let (Some(repo), Some(pr)) = (&repo, &pr) {
			let (owner, project) = split_repo(repo)?;
			return Ok((owner.to_owned(), project.to_owned(), *pr));
		}
		if repo.is_some() != pr.is_some() {
			return Err(AppError::message(format_error(
				"Missing fields",
				Some("Provide both `repository` and `pr`, or neither (inside a PR thread)."),
			)));
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

		Ok((pr_msg.owner, pr_msg.project, pr_msg.pr))
	}

	/// Resolve a reviewer input (`GitHub` login or `Discord` ID) to a `GitHub` login.
	async fn resolve_reviewer_login(&self, reviewer: &ReviewerInput) -> Result<String, AppError> {
		match reviewer {
			ReviewerInput::GitHubLogin(login) => Ok(login.clone()),
			ReviewerInput::DiscordMention(discord_id) => {
				self.resolve_discord_user_login(*discord_id).await
			}
		}
	}

	/// Resolve a `Discord` user ID to its linked `GitHub` login, backed by the
	/// same `user_links` table that `/link` writes to.
	async fn resolve_discord_user_login(&self, discord_id: u64) -> Result<String, AppError> {
		self.user_store
			.fetch_by_discord_id(discord_id)
			.await?
			.map(|link| link.github_login)
			.ok_or_else(|| {
				AppError::message(format_error(
					"Discord user has not linked their GitHub account",
					Some("Ask them to run `/link` first."),
				))
			})
	}

	/// Render the assigned reviewer as a `Discord` mention when they have one
	/// linked, preferring the plain `GitHub` login otherwise. Never emits a
	/// mention that would ping the actor themself.
	async fn resolve_reviewer_display(
		&self,
		reviewer_login: &str,
		actor_discord_id: u64,
	) -> String {
		let discord_ids = self
			.user_store
			.fetch_by_github_logins(&[reviewer_login.to_owned()])
			.await
			.unwrap_or_default();

		match discord_ids.get(reviewer_login) {
			Some(id) if *id == actor_discord_id => reviewer_login.to_owned(),
			Some(id) => format!("<@{id}>"),
			None => reviewer_login.to_owned(),
		}
	}

	/// Sends an assign audit message to every saved PR message's thread.
	async fn broadcast_audit(&self, http: &Http, ctx: &AssignContext) {
		let pr_messages = match self
			.pr_store
			.fetch_all_by_repository_and_pr(&ctx.owner, &ctx.project, ctx.pr)
			.await
		{
			Ok(messages) => messages,
			Err(e) => {
				error!(
					error = %e,
					owner = %ctx.owner,
					project = %ctx.project,
					pr = ctx.pr,
					"failed to fetch PR messages for audit"
				);
				return;
			}
		};

		if pr_messages.is_empty() {
			warn!(
				owner = %ctx.owner,
				project = %ctx.project,
				pr = ctx.pr,
				"no PR message records found for audit post"
			);
			return;
		}

		let msg = match ctx.action {
			AssignAction::Assign => {
				format!(
					"👥 **{}** requested review from **{}**",
					ctx.actor, ctx.reviewer_display
				)
			}
			AssignAction::Unassign => format!(
				"👤 **{}** removed review request from **{}**",
				ctx.actor, ctx.reviewer_display
			),
		};

		broadcast_to_threads(http, &pr_messages, &msg).await;
	}
}

/// Request or remove a review on `GitHub`, matching the requested action.
async fn apply_review_change(
	client: &Octocrab,
	ctx: &AssignContext,
	is_requested: bool,
) -> Result<(), AppError> {
	match ctx.action {
		AssignAction::Assign => {
			if is_requested {
				return Err(AppError::message(format_error(
					"Review has already been requested by that user",
					None,
				)));
			}
			github::api::pull_requests::assign_reviewer(
				client,
				&ctx.owner,
				&ctx.project,
				ctx.pr,
				&ctx.reviewer_login,
			)
			.await
			.map_err(|_| {
				AppError::message(format_error(
					"Could not request a review from that user",
					Some("Verify the reviewer has write/pull access and is not the PR author."),
				))
			})
		}
		AssignAction::Unassign => {
			if !is_requested {
				return Err(AppError::message(format_error(
					"That user is not currently a requested reviewer",
					Some("They may have already submitted a review or were never requested."),
				)));
			}
			github::api::pull_requests::unassign_reviewer(
				client,
				&ctx.owner,
				&ctx.project,
				ctx.pr,
				&ctx.reviewer_login,
			)
			.await
			.map_err(|_| {
				AppError::message(format_error(
					"Could not remove the reviewer",
					Some("Verify the user was actually requested and the PR number is correct."),
				))
			})
		}
	}
}
