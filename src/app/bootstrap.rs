//! Application construction and lifecycle startup.

use super::observability::prometheus;
use crate::app::observability::renderer::MetricsRenderer;
use crate::app::observability::MetricsRecorder;
use crate::app::queue_consumer::run_queue_consumer;
use crate::app::server::serve_http;
use crate::app::shutdown::shutdown_signal;
use crate::broker::rabbitmq::publisher::GITHUB_EVENTS_EXCHANGE;
use crate::broker::rabbitmq::{RabbitMqConnection, RabbitMqConsumer, RabbitMqPublisher};
use crate::broker::{MessageConsumer, MessagePublisher};
use crate::config::{EnvConfig, Environment};
use crate::db::create_stores;
use crate::error::AppError;
use crate::github::webhook::events::installation::InstallationEventHandler;
use crate::github::webhook::events::installation_repositories::InstallationRepositoriesEventHandler;
use crate::github::webhook::events::issue_comment::IssueCommentEventHandler;
use crate::github::webhook::events::pull_request::PullRequestEventHandler;
use crate::github::webhook::events::review::ReviewEventHandler;
use crate::github::webhook::router::{WebhookEventHandler, WebhookRouter};
use crate::service::discord::assign::AssignService;
use crate::service::discord::health::HealthService;
use crate::service::discord::link::UserLinkService;
use crate::service::discord::subscribe::SubscribeService;
use crate::service::github::installation::InstallationService;
use crate::service::github::installation_repositories::InstallationRepositoriesService;
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
	queue_consumer: Arc<dyn MessageConsumer>,
	port: u16,
	internal_port: u16,
	metrics_recorder: Arc<dyn MetricsRecorder>,
	metrics_renderer: Arc<dyn MetricsRenderer>,
}

impl Application {
	pub async fn build(env_config: EnvConfig) -> Result<Self, AppError> {
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

		// One AMQP connection and a publisher and consumer channel on a TCP connection
		let rabbitmq_connection = RabbitMqConnection::connect(&env_config.rabbitmq_url).await?;
		let publish_channel = rabbitmq_connection.create_channel().await?;
		let consume_channel = rabbitmq_connection.create_channel().await?;

		let publisher: Arc<dyn MessagePublisher> = Arc::new(
			RabbitMqPublisher::new(publish_channel, GITHUB_EVENTS_EXCHANGE.to_string()).await?,
		);
		let queue_consumer: Arc<dyn MessageConsumer> = Arc::new(RabbitMqConsumer::new(
			consume_channel,
			format!("{}-consumer", Environment::from(env_config.local_dev)),
			env_config.rabbitmq_prefetch,
		));

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

		let installation_repositories_service = Arc::new(InstallationRepositoriesService::new(
			Arc::clone(&stores.subscriptions),
		));

		let webhook_router = Arc::new(WebhookRouter::new(
			env_config.github_webhook_secret.clone(),
			Self::build_webhook_handlers(
				pull_request_service,
				review_service,
				issue_comment_service,
				installation_service,
				installation_repositories_service,
			),
			Arc::clone(&metrics_recorder),
			publisher,
		));

		Ok(Self {
			discord_client,
			webhook_router,
			queue_consumer,
			port: env_config.port,
			internal_port: env_config.internal_port,
			metrics_recorder,
			metrics_renderer,
		})
	}

	/// Constructs the webhook event handlers from their services. Lives in the
	/// composition root so the router stays agnostic of concrete services.
	fn build_webhook_handlers(
		pull_request_service: Arc<PullRequestService>,
		review_service: Arc<ReviewService>,
		issue_comment_service: Arc<IssueCommentService>,
		installation_service: Arc<InstallationService>,
		installation_repositories_service: Arc<InstallationRepositoriesService>,
	) -> [Arc<dyn WebhookEventHandler>; 5] {
		[
			Arc::new(PullRequestEventHandler::new(pull_request_service)),
			Arc::new(ReviewEventHandler::new(review_service)),
			Arc::new(IssueCommentEventHandler::new(issue_comment_service)),
			Arc::new(InstallationEventHandler::new(installation_service)),
			Arc::new(InstallationRepositoriesEventHandler::new(
				installation_repositories_service,
			)),
		]
	}

	pub async fn run(mut self) -> Result<(), AppError> {
		let shard_manager = self.discord_client.shard_manager.clone();

		let mut http = spawn(serve_http(
			self.port,
			self.internal_port,
			Arc::clone(&self.webhook_router),
			self.metrics_renderer,
			self.metrics_recorder,
		));
		let mut discord = spawn(async move { self.discord_client.start().await });
		let mut queue = spawn(run_queue_consumer(self.queue_consumer, self.webhook_router));

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
			res = &mut queue => {
				error!("Queue consumer exited: {res:?}");
				return Err(AppError::Internal(anyhow!(
					"Queue consumer exited unexpectedly: {res:?}"
				)));
			}
			() = shutdown_signal() => {
				info!("shutdown signal received, stopping Discord client");
				shard_manager.shutdown_all().await;
			}
		}

		let _ = tokio::join!(http, discord, queue);
		Ok(())
	}
}
