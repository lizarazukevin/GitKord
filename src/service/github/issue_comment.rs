//! Service layer for issue‑comment events.

use serenity::all::Http;
use std::sync::Arc;
use tracing::info;

use crate::error::AppError;
use crate::github::api::client::GitHubClient;
use crate::github::webhook::events::issue_comment::IssueCommentPayload;
use crate::models::pr_message::PrStore;
use crate::models::subscription::SubscriptionStore;
use crate::models::user_link::UserStore;
use crate::service::github::pr_messages;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueCommentAction {
	Created,
	Other,
}

pub struct IssueCommentRequest {
	pub action: IssueCommentAction,
	pub is_pull_request: bool,
	pub owner: String,
	pub project: String,
	pub pr_number: u64,
}

impl IssueCommentRequest {
	pub fn from_payload(payload: IssueCommentPayload) -> Self {
		Self {
			action: match payload.action.as_str() {
				"created" => IssueCommentAction::Created,
				_ => IssueCommentAction::Other,
			},
			is_pull_request: payload.issue.pull_request.is_some(),
			owner: payload.repository.owner.login,
			project: payload.repository.name,
			pr_number: payload.issue.number,
		}
	}
}

pub struct IssueCommentService {
	pr_store: Arc<dyn PrStore>,
	sub_store: Arc<dyn SubscriptionStore>,
	user_store: Arc<dyn UserStore>,
	github: Arc<GitHubClient>,
	http: Arc<Http>,
}

impl IssueCommentService {
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

	/// Refresh the PR message in subscribed channels when a PR comment is created.
	pub async fn handle(&self, req: IssueCommentRequest) -> Result<(), AppError> {
		// Skip plain issues and non‑created actions
		if !req.is_pull_request || !matches!(req.action, IssueCommentAction::Created) {
			return Ok(());
		}

		let message_data = pr_messages::load_pr_message_data(
			&self.github,
			self.sub_store.as_ref(),
			self.user_store.as_ref(),
			&req.owner,
			&req.project,
			req.pr_number,
		)
		.await?;

		let repository = format!("{}/{}", req.owner, req.project);
		let pr_messages = pr_messages::update_all_pr_messages(
			self.pr_store.as_ref(),
			&self.http,
			&repository,
			req.pr_number,
			&message_data,
		)
		.await?;

		if pr_messages.is_empty() {
			info!(repo = %repository, pr = req.pr_number, "no stored messages to update");
		} else {
			info!(repo = %repository, pr = req.pr_number, "PR message refreshed after new comment");
		}

		Ok(())
	}
}
