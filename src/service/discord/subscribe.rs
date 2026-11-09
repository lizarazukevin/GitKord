//! Service layer for `/subscribe` and `/unsubscribe` commands.

use std::sync::Arc;
use tracing::{error, info};

use crate::config::{WebhookRegistrationConfig, APP_NAME, GITHUB_APP_URL};
use crate::error::{format_error, AppError};
use crate::github;
use crate::github::api::client::GitHubClient;
use crate::github::api::pull_requests::split_repo;
use crate::models::subscription::{CreateSubscriptionOutcome, Subscription, SubscriptionStore};
use crate::service::entitlement::EntitlementService;
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
	entitlement_service: Arc<EntitlementService>,
	github: Arc<GitHubClient>,
	config: WebhookRegistrationConfig,
	basic_subscription_cap: u32,
	premium_subscription_cap: u32,
}

impl SubscribeService {
	pub fn new(
		sub_store: Arc<dyn SubscriptionStore>,
		entitlement_service: Arc<EntitlementService>,
		github: Arc<GitHubClient>,
		config: WebhookRegistrationConfig,
		basic_subscription_cap: u32,
		premium_subscription_cap: u32,
	) -> Self {
		Self {
			sub_store,
			entitlement_service,
			github,
			config,
			basic_subscription_cap,
			premium_subscription_cap,
		}
	}

	/// For subscribe in production: verifies the GitHub App is installed.
	/// For subscribe in local dev: registers a webhook on the repository.
	/// For unsubscribe: just removes the database record.
	pub async fn handle(&self, req: SubscribeRequest) -> Result<String, AppError> {
		let (owner, project) = split_repo(&req.repo)?;

		let installation_id = match req.action {
			SubscribeAction::Subscribe => self.resolve_installation_id(&req.repo).await?,
			SubscribeAction::Unsubscribe => 0,
		};

		match req.action {
			SubscribeAction::Subscribe => {
				let subscription = Subscription {
					owner: owner.to_owned(),
					project: project.to_owned(),
					guild_id: req.guild_id,
					channel_id: req.channel_id,
					installation_id,
					created_at: Utc::now(),
					updated_at: Utc::now(),
					created_by: Some(req.discord_id.to_string()),
					updated_by: Some(req.discord_id.to_string()),
				};

				let plan = self
					.entitlement_service
					.resolve_plan(req.discord_id)
					.await?;
				let plan_cap = plan
					.max_subscriptions(self.basic_subscription_cap, self.premium_subscription_cap);

				match self
					.sub_store
					.create_within_limit(subscription, plan_cap)
					.await?
				{
					CreateSubscriptionOutcome::Created => {
						info!(repo = %req.repo, "channel subscribed");
						Ok(format!(
							"This channel will now receive PR updates for **{}**.",
							req.repo
						))
					}
					CreateSubscriptionOutcome::AlreadySubscribed => {
						info!(repo = %req.repo, "channel already subscribed");
						Ok(format!(
							"This channel is already subscribed to **{}**.",
							req.repo
						))
					}
					CreateSubscriptionOutcome::LimitReached { used, cap } => {
						info!(repo = %req.repo, used, cap, "subscription limit reached");
						Err(limit_reached_error(&req.repo, used, cap))
					}
				}
			}
			SubscribeAction::Unsubscribe => {
				self.sub_store
					.delete(owner, project, req.guild_id, req.channel_id)
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

/// User-facing refusal for a plan cap rejection.
fn limit_reached_error(repo: &str, used: u32, cap: u32) -> AppError {
	AppError::message(format_error(
		&format!("You've reached the Basic plan limit of {cap} repository subscriptions."),
		Some(&format!(
			"You're using **{used}** of **{cap}**. Existing subscriptions are kept, including any that were created before this limit. To subscribe to **{repo}**, run `/unsubscribe` for a repository you no longer need.\n\n-# *Premium plans with a higher limit are coming soon.*"
		)),
	))
}
