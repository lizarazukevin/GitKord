//! Service layer for `/subscribe` and `/unsubscribe` commands.

use std::sync::Arc;
use tracing::{error, info};

use crate::config::{WebhookRegistrationConfig, APP_NAME, GITHUB_APP_URL};
use crate::error::{format_error, AppError};
use crate::github;
use crate::github::api::client::GitHubClient;
use crate::github::api::pull_requests::split_repo;
use crate::models::subscription::{Subscription, SubscriptionStore};
use chrono::Utc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscribeAction {
	Subscribe,
	Unsubscribe,
}

pub struct SubscribeRequest {
	pub repo: String,
	pub action: SubscribeAction,
	pub guild_id: u64,
	pub channel_id: u64,
	pub discord_id: u64,
}

pub struct SubscribeService {
	sub_store: Arc<dyn SubscriptionStore>,
	github: Arc<GitHubClient>,
	config: WebhookRegistrationConfig,
}

impl SubscribeService {
	pub fn new(
		sub_store: Arc<dyn SubscriptionStore>,
		github: Arc<GitHubClient>,
		config: WebhookRegistrationConfig,
	) -> Self {
		Self {
			sub_store,
			github,
			config,
		}
	}

	/// For subscribe in production: verifies the GitHub App is installed.
	/// For subscribe in local dev: registers a webhook on the repository.
	/// For unsubscribe: just removes the database record.
	pub async fn handle(&self, req: SubscribeRequest) -> Result<String, AppError> {
		let installation_id = match req.action {
			SubscribeAction::Subscribe => self.resolve_installation_id(&req.repo).await?,
			SubscribeAction::Unsubscribe => 0,
		};

		match req.action {
			SubscribeAction::Subscribe => {
				self.sub_store
					.upsert(Subscription {
						repository: req.repo.clone(),
						guild_id: req.guild_id,
						channel_id: req.channel_id,
						installation_id,
						created_at: Utc::now(),
						updated_at: Utc::now(),
						created_by: Some(req.discord_id.to_string()),
						updated_by: Some(req.discord_id.to_string()),
					})
					.await?;

				info!(repo = %req.repo, "channel subscribed");

				Ok(format!(
					"This channel will now receive PR updates for **{}**.",
					req.repo
				))
			}
			SubscribeAction::Unsubscribe => {
				self.sub_store
					.delete(&req.repo, req.guild_id, req.channel_id)
					.await?;

				info!(repo = %req.repo, "channel unsubscribed");

				Ok(format!(
					"This channel will no longer receive PR updates for **{}**.",
					req.repo
				))
			}
		}
	}

	/// Resolve the installation ID for a repository.
	///
	/// In production this queries the GitHub API. In local dev it registers
	/// a webhook directly on the repository and returns a placeholder ID.
	async fn resolve_installation_id(&self, repo: &str) -> Result<u64, AppError> {
		if self.config.local_dev {
			return self.register_dev_webhook(repo).await;
		}

		let (owner, project) = split_repo(repo)?;

		github::api::installations::fetch_repository_installation_id(
			self.github.authenticated(),
			owner,
			project,
		)
		.await
		.map_err(|e| {
			error!(error = %e, repo, "failed to get installation ID");
			AppError::message(format_error(
				format!("{APP_NAME} is not installed on that repository").as_ref(),
				Some(
					format!("Install it at {GITHUB_APP_URL} first, then run `/subscribe` again.")
						.as_ref(),
				),
			))
		})
	}

	/// Register a webhook on the repository (local dev only).
	async fn register_dev_webhook(&self, repo: &str) -> Result<u64, AppError> {
		let (owner, project) = split_repo(repo)?;

		let payload_url = format!("https://{}/github/webhook", self.config.public_domain);

		github::api::webhooks::register(
			self.github.authenticated(),
			owner,
			project,
			&payload_url,
			&self.config.github_webhook_secret,
		)
		.await
		.map_err(|e| {
			error!(error = %e, repo, "failed to register webhook");
			AppError::message(format_error(
				format!("Could not register webhook for {repo}").as_ref(),
				Some("Make sure your PAT has admin access to the repository."),
			))
		})?;

		info!(repo, "webhook registered for local dev");
		Ok(0)
	}
}
