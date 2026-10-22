//! Service layer for pull‑request events.

use serenity::all::{ChannelId, Http};
use std::collections::HashSet;
use std::sync::Arc;
use tracing::{error, info};

use crate::discord::messaging::audit::{
	broadcast_to_threads, post_commit_push, post_lifecycle_pr_update,
};
use crate::discord::messaging::messages::{post_pull_request_message, rollback_posted_pr_message};
use crate::error::AppError;
use crate::github::api::client::GitHubClient;
use crate::github::webhook::events::models::PullRequestInfo;
use crate::github::webhook::events::pull_request::PullRequestPayload;
use crate::models::pr_message::{PrMessage, PrStore};
use crate::models::subscription::SubscriptionStore;
use crate::models::user_link::UserStore;
use crate::service::github::pr_messages::{load_pr_message_data, update_all_pr_messages};
use chrono::Utc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullRequestAction {
	Opened,
	Closed,
	Reopened,
	Synchronize,
	Edited,
	ReviewRequested,
	ReviewRequestRemoved,
	Other,
}

impl PullRequestAction {
	pub const fn as_str(self) -> &'static str {
		match self {
			Self::Opened => "opened",
			Self::Closed => "closed",
			Self::Reopened => "reopened",
			Self::Synchronize => "synchronize",
			Self::Edited => "edited",
			Self::ReviewRequested => "review_requested",
			Self::ReviewRequestRemoved => "review_request_removed",
			Self::Other => "other",
		}
	}
}

pub struct PullRequestRequest {
	pub action: PullRequestAction,
	pub owner: String,
	pub project: String,
	pub pr: PullRequestInfo,
	pub sender: String,
	/// Whether the event's sender is a bot/app rather than a person, used to
	/// skip a webhook audit for changes `GitKord`'s own GitHub API calls trigger.
	pub sender_is_bot: bool,
}

impl PullRequestRequest {
	pub fn from_payload(payload: PullRequestPayload) -> Self {
		Self {
			action: match payload.action.as_str() {
				"opened" => PullRequestAction::Opened,
				"closed" => PullRequestAction::Closed,
				"reopened" => PullRequestAction::Reopened,
				"synchronize" => PullRequestAction::Synchronize,
				"edited" => PullRequestAction::Edited,
				"review_requested" => PullRequestAction::ReviewRequested,
				"review_request_removed" => PullRequestAction::ReviewRequestRemoved,
				_ => PullRequestAction::Other,
			},
			owner: payload.repository.owner.login,
			project: payload.repository.name,
			pr: payload.pull_request,
			sender: payload.sender.login.clone(),
			sender_is_bot: payload.sender.is_bot(),
		}
	}
}

pub struct PullRequestService {
	pr_store: Arc<dyn PrStore>,
	sub_store: Arc<dyn SubscriptionStore>,
	user_store: Arc<dyn UserStore>,
	github: Arc<GitHubClient>,
	http: Arc<Http>,
}

impl PullRequestService {
	pub fn new(
		pr_store: Arc<dyn PrStore>,
		sub_store: Arc<dyn SubscriptionStore>,
		user_store: Arc<dyn UserStore>,
		github: Arc<GitHubClient>,
		http: Arc<Http>,
	) -> Self {
		Self {
			pr_store,
			sub_store,
			user_store,
			github,
			http,
		}
	}

	/// Route a PR event to the handler for its action.
	pub async fn handle(&self, req: PullRequestRequest) -> Result<(), AppError> {
		match req.action {
			PullRequestAction::Opened => self.handle_opened(req).await,
			PullRequestAction::Synchronize => self.handle_synchronize(req).await,
			PullRequestAction::Closed | PullRequestAction::Reopened => {
				self.handle_lifecycle_change(req).await
			}
			PullRequestAction::ReviewRequested | PullRequestAction::ReviewRequestRemoved => {
				self.handle_review_request_change(req).await
			}
			PullRequestAction::Edited => self.refresh_and_skip_audit(req).await,
			PullRequestAction::Other => Ok(()),
		}
	}

	/// A new PR was opened, post the initial message to every subscribed channel.
	/// Side effect of [`post_pull_request_message`] is creating an audit thread.
	async fn handle_opened(&self, req: PullRequestRequest) -> Result<(), AppError> {
		let Some(message_data) = load_pr_message_data(
			&self.github,
			self.sub_store.as_ref(),
			self.user_store.as_ref(),
			&req.owner,
			&req.project,
			req.pr.number,
		)
		.await?
		else {
			return Ok(());
		};

		// A retry can re-run this after a partial failure, so skip channels that
		// already got a message instead of posting them a duplicate.
		let existing = self
			.pr_store
			.fetch_all_by_repository_and_pr(&req.owner, &req.project, req.pr.number)
			.await?;
		let already_posted: HashSet<u64> = existing.iter().map(|msg| msg.channel_id).collect();

		let subscriptions = self
			.sub_store
			.fetch_all_by_owner_project(&req.owner, &req.project)
			.await?;
		for sub in &subscriptions {
			if already_posted.contains(&sub.channel_id) {
				info!(
					channel_id = sub.channel_id,
					"PR message already posted, skipping duplicate"
				);
				continue;
			}

			let posted = post_pull_request_message(
				&self.http,
				ChannelId::from(sub.channel_id),
				&message_data,
			)
			.await?;

			if let Err(save_err) = self
				.pr_store
				.upsert(PrMessage {
					owner: req.owner.clone(),
					project: req.project.clone(),
					pr: req.pr.number,
					channel_id: sub.channel_id,
					message_id: posted.message_id,
					thread_id: posted.thread_id,
					created_at: Utc::now(),
					updated_at: Utc::now(),
					created_by: Some(req.sender.clone()),
					updated_by: Some(req.sender.clone()),
				})
				.await
			{
				// Message delivered to `Discord` but did not persist.
				// Roll back the message to avoid stacking.
				rollback_posted_pr_message(
					&self.http,
					sub.channel_id,
					posted.message_id,
					posted.thread_id,
				)
				.await;
				return Err(save_err);
			}
		}

		Ok(())
	}

	/// New commits were pushed. Refresh the main message and append a commit‑push
	/// audit entry to every thread.
	async fn handle_synchronize(&self, req: PullRequestRequest) -> Result<(), AppError> {
		let pr_messages = self
			.update_pr_messages(&req.owner, &req.project, req.pr.number)
			.await?;

		let sha = req.pr.head.sha;
		for msg in &pr_messages {
			if let Err(e) = post_commit_push(&self.http, msg.thread_id, &req.sender, &sha).await {
				error!(error = %e, thread_id = msg.thread_id, "failed to post commit push");
			}
		}

		Ok(())
	}

	/// The PR was closed or reopened. Refresh the main message and append a
	/// lifecycle audit entry.
	async fn handle_lifecycle_change(&self, req: PullRequestRequest) -> Result<(), AppError> {
		let pr_messages = self
			.update_pr_messages(&req.owner, &req.project, req.pr.number)
			.await?;

		for message in &pr_messages {
			if let Err(e) = post_lifecycle_pr_update(
				&self.http,
				message.thread_id,
				req.pr.number,
				req.action.as_str(),
				req.pr.merged,
			)
			.await
			{
				error!(error = %e, thread_id = message.thread_id, "failed to post lifecycle audit");
			}
		}

		Ok(())
	}

	/// A reviewer was requested or unrequested on the PR, event from website.
	async fn handle_review_request_change(&self, req: PullRequestRequest) -> Result<(), AppError> {
		let prev = self
			.pr_store
			.fetch_requested_reviewers(&req.owner, &req.project, req.pr.number)
			.await?;
		let next: Vec<String> = req
			.pr
			.requested_reviewers
			.iter()
			.map(|reviewer| reviewer.login.clone())
			.collect();

		let pr_messages = self
			.update_pr_messages(&req.owner, &req.project, req.pr.number)
			.await?;

		if req.sender_is_bot || pr_messages.is_empty() {
			return Ok(());
		}

		let Some(msg) = self
			.review_request_audit(req.action, &req.sender, &prev, &next)
			.await
		else {
			return Ok(());
		};

		broadcast_to_threads(&self.http, &pr_messages, &msg).await;

		Ok(())
	}

	/// Build the audit line for a website-initiated reviewer change by
	/// reconciling the stored baseline against the event's reviewer list.
	async fn review_request_audit(
		&self,
		action: PullRequestAction,
		sender: &str,
		prev: &[String],
		next: &[String],
	) -> Option<String> {
		let (added, removed) = diff_reviewers(prev, next);

		let affected = match action {
			PullRequestAction::ReviewRequested => added,
			PullRequestAction::ReviewRequestRemoved => removed,
			_ => return None,
		};

		// With no stored baseline assume the change covers the current list for requests.
		// With a baseline but no diff, the event is either a duplicate or stale delivery.
		let affected = if affected.is_empty() {
			if prev.is_empty() {
				match action {
					PullRequestAction::ReviewRequested => next.to_vec(),
					_ => return None,
				}
			} else {
				return None;
			}
		} else {
			affected
		};

		if affected.is_empty() {
			return None;
		}

		let names = self.reviewer_display(&affected).await;
		let reviewer_list = names.join(", ");

		match action {
			PullRequestAction::ReviewRequested => Some(format!(
				"👥 **{sender}** requested review from **{reviewer_list}**"
			)),
			PullRequestAction::ReviewRequestRemoved => Some(format!(
				"👤 **{sender}** removed review request from **{reviewer_list}**"
			)),
			_ => None,
		}
	}

	/// Render each login as its `Discord` mention when linked, else the login.
	async fn reviewer_display(&self, logins: &[String]) -> Vec<String> {
		let discord_ids = self
			.user_store
			.fetch_by_github_logins(logins)
			.await
			.unwrap_or_default();

		logins
			.iter()
			.map(|login| {
				discord_ids
					.get(login)
					.map_or_else(|| login.clone(), |id| format!("<@{id}>"))
			})
			.collect()
	}

	/// Actions that only change the visible PR message data.
	///
	/// Refresh the message without any audit entry. Either handled silently
	/// or no planned audit entry for event.
	async fn refresh_and_skip_audit(&self, req: PullRequestRequest) -> Result<(), AppError> {
		self.update_pr_messages(&req.owner, &req.project, req.pr.number)
			.await?;
		Ok(())
	}

	/// Refresh the PR status message in every subscribed channel.
	/// Returns the list of PR message records that were updated.
	///
	/// Delegates the fetch-and-edit to [`update_all_pr_messages`] so the
	/// update loop lives in exactly one place.
	async fn update_pr_messages(
		&self,
		owner: &str,
		project: &str,
		pr_number: u64,
	) -> Result<Vec<PrMessage>, AppError> {
		let Some(message_data) = load_pr_message_data(
			&self.github,
			self.sub_store.as_ref(),
			self.user_store.as_ref(),
			owner,
			project,
			pr_number,
		)
		.await?
		else {
			return Ok(vec![]);
		};

		update_all_pr_messages(
			self.pr_store.as_ref(),
			&self.http,
			owner,
			project,
			pr_number,
			&message_data,
		)
		.await
	}
}

/// Returns the logins added to and removed from `next` relative to `prev`.
fn diff_reviewers(prev: &[String], next: &[String]) -> (Vec<String>, Vec<String>) {
	let added = next
		.iter()
		.filter(|login| !prev.contains(login))
		.cloned()
		.collect();
	let removed = prev
		.iter()
		.filter(|login| !next.contains(login))
		.cloned()
		.collect();

	(added, removed)
}
