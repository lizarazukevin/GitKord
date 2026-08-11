//! Application construction and lifecycle startup.

use super::observability::prometheus;
use crate::app::observability::renderer::MetricsRenderer;
use crate::app::observability::MetricsRecorder;
use crate::app::server::serve_http;
use crate::app::shutdown::shutdown_signal;
use crate::config::{EnvConfig, Environment};
use crate::db::create_stores;
use crate::error::AppError;
use crate::github::webhook::router::WebhookRouter;
use crate::service::discord::assign::AssignService;
use crate::service::discord::health::HealthService;
use crate::service::discord::link::UserLinkService;
use crate::service::discord::subscribe::SubscribeService;
use crate::service::github::installation::InstallationService;
use crate::service::github::issue_comment::IssueCommentService;
use crate::service::github::pull_request::PullRequestService;
use crate::service::github::review::ReviewService;
use crate::{discord, github};
use anyhow::anyhow;
use std::sync::Arc;
use tokio::{select, spawn};
use tracing::{error, info};

pub(super) struct Application {
	discord_client: serenity::Client,
	webhook_router: Arc<WebhookRouter>,
	port: u16,
	internal_port: u16,
	#[expect(dead_code)]
	metrics_recorder: Arc<dyn MetricsRecorder>,
	metrics_renderer: Arc<dyn MetricsRenderer>,
}

impl Application {
	pub async fn build() -> Result<Self, AppError> {
		let env_config = EnvConfig::from_env()?;
		let webhook_registration_config = env_config.webhook_registration_config();

		let gh_client = Arc::new(github::api::client::GitHubClient::new(
			env_config.github_app_id,
			&env_config.github_app_private_key,
			&env_config.github_token,
			env_config.local_dev,
		)?);

		let stores = create_stores(&env_config.database_url).await?;

		let environment = Environment::from(env_config.local_dev);
		let (recorder, exporter) = prometheus::init(&environment.to_string())?;
		let metrics_recorder: Arc<dyn MetricsRecorder> = Arc::new(recorder);
		let metrics_renderer: Arc<dyn MetricsRenderer> = Arc::new(exporter);

		let assign_service = Arc::new(AssignService::new(
			Arc::clone(&stores.prs),
			Arc::clone(&stores.subscriptions),
			Arc::clone(&stores.users),
			Arc::clone(&gh_client),
		));

		let subscribe_service = Arc::new(SubscribeService::new(
			Arc::clone(&stores.subscriptions),
			Arc::clone(&gh_client),
			webhook_registration_config.clone(),
		));

		let link_service = Arc::new(UserLinkService::new(
			Arc::clone(&stores.users),
			Arc::clone(&gh_client),
		));

		let health_service = Arc::new(HealthService::new(Arc::clone(&gh_client)));

		let module_registry = discord::commands::registry::build_registry(
			assign_service,
			subscribe_service,
			link_service,
			health_service,
		);

		let (discord_client, http) = discord::client::build(
			&env_config.discord_token,
			module_registry,
			Arc::clone(&metrics_recorder),
		)
		.await?;

		let pull_request_service = Arc::new(PullRequestService::new(
			Arc::clone(&stores.prs),
			Arc::clone(&stores.subscriptions),
			Arc::clone(&stores.users),
			Arc::clone(&gh_client),
			Arc::clone(&http),
		));

		let review_service = Arc::new(ReviewService::new(
			Arc::clone(&stores.prs),
			Arc::clone(&stores.subscriptions),
			Arc::clone(&stores.users),
			Arc::clone(&gh_client),
			Arc::clone(&http),
		));

		let issue_comment_service = Arc::new(IssueCommentService::new(
			Arc::clone(&stores.prs),
			Arc::clone(&stores.subscriptions),
			Arc::clone(&stores.users),
			Arc::clone(&gh_client),
			Arc::clone(&http),
		));

		let installation_service =
			Arc::new(InstallationService::new(Arc::clone(&stores.subscriptions)));

		let webhook_router = Arc::new(WebhookRouter::new(
			env_config.github_webhook_secret.clone(),
			pull_request_service,
			review_service,
			issue_comment_service,
			installation_service,
		));

		Ok(Self {
			discord_client,
			webhook_router,
			port: env_config.port,
			internal_port: env_config.internal_port,
			metrics_recorder,
			metrics_renderer,
		})
	}

	pub async fn run(mut self) -> Result<(), AppError> {
		let shard_manager = self.discord_client.shard_manager.clone();

		let mut http = spawn(serve_http(
			self.port,
			self.internal_port,
			self.webhook_router,
			self.metrics_renderer,
		));
		let mut discord = spawn(async move { self.discord_client.start().await });

		select! {
			res = &mut http => {
				error!("HTTP server exited: {res:?}");
				return Err(AppError::Internal(anyhow!(
					"HTTP server exited unexpectedly: {res:?}"
				)));
			}
			res = &mut discord => {
				error!("Discord client exited: {res:?}");
				return Err(AppError::Internal(anyhow!(
					"Discord client exited unexpectedly: {res:?}"
				)));
			}
			() = shutdown_signal() => {
				info!("shutdown signal received, stopping Discord client");
				shard_manager.shutdown_all().await;
			}
		}

		let _ = tokio::join!(http, discord);
		Ok(())
	}
}
