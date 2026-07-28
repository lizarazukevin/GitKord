//! Service layer for pull‑request review events.

use serenity::all::Http;
use std::sync::Arc;
use tracing::{error, info};

use crate::discord::messaging::audit::post_review_verdict;
use crate::error::AppError;
use crate::github::api::client::GitHubClient;
use crate::github::webhook::events::review::PullRequestReviewPayload;
use crate::models::pr_message::PrStore;
use crate::models::subscription::SubscriptionStore;
use crate::models::user_link::UserStore;
use crate::service::github::pr_messages;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewAction {
	Submitted,
	Dismissed,
	Other,
}

pub struct ReviewRequest {
	pub action: ReviewAction,
	pub owner: String,
	pub project: String,
	pub pr_number: u64,
	pub reviewer_login: String,
	pub review_state: String,
	pub review_emoji: String,
}

impl ReviewRequest {
	pub fn from_payload(payload: PullRequestReviewPayload) -> Self {
		Self {
			action: match payload.action.as_str() {
				"submitted" => ReviewAction::Submitted,
				"dismissed" => ReviewAction::Dismissed,
				_ => ReviewAction::Other,
			},
			owner: payload.repository.owner.login,
			project: payload.repository.name,
			pr_number: payload.pull_request.number,
			reviewer_login: payload.review.user.login.clone(),
			review_state: payload.review.state.to_string(),
			review_emoji: payload.review.verdict_emoji().to_string(),
		}
	}
}

pub struct ReviewService {
	pr_store: Arc<dyn PrStore>,
	sub_store: Arc<dyn SubscriptionStore>,
	user_store: Arc<dyn UserStore>,
	github: Arc<GitHubClient>,
	http: Arc<Http>,
}

impl ReviewService {
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

	/// Refresh the PR message and post a review-verdict audit entry per thread.
	pub async fn handle(&self, req: ReviewRequest) -> Result<(), AppError> {
		if matches!(req.action, ReviewAction::Other) {
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

		for message in &pr_messages {
			if let Err(e) = post_review_verdict(
				&self.http,
				message.thread_id,
				&req.reviewer_login,
				&req.review_state,
				&req.review_emoji,
			)
			.await
			{
				error!(error = %e, thread_id = message.thread_id, "failed to post review verdict");
			}
		}

		info!(
			repo = %repository,
			pr = req.pr_number,
			reviewer = %req.reviewer_login,
			action = ?req.action,
			"review event processed"
		);

		Ok(())
	}
}
