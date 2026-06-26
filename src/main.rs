#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]
#![allow(clippy::multiple_crate_versions)] // transitive deps — not in our control

//! `GitKord` entrypoint.
//!
//! Two tasks run concurrently for the lifetime of the process:
//! - Axum HTTP server — receives GitHub webhook payloads
//! - Serenity Discord client — handles gateway events and slash commands
//!
//! Both are spawned as Tokio tasks so neither blocks the other.
//! If either task exits unexpectedly, the process exits with a non-zero code
//! so Railway (or any supervisor) knows to restart it.

mod config;
mod constants;
mod db;
mod discord;
mod error;
mod github;

use crate::constants::APP_NAME;
use crate::db::postgres::pr_channel_messages::PostgresPrChannelMessageStore;
use crate::db::postgres::schema::connect;
use crate::db::postgres::subscriptions::PostgresSubscriptionStore;
use crate::db::postgres::user_links::PgPoolUserLinkStore;
use crate::discord::context::AppState;
use crate::github::client::GitHubClient;
use crate::github::context::WebhookState;
use anyhow::Result;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let config = config::Config::from_env()?;
    info!("{APP_NAME} starting");

    let github = Arc::new(GitHubClient::build(
        config.github_app_id,
        &config.github_app_private_key,
        config.local_dev,
        &config.github_token,
    )?);

    let pool = connect(&config.database_url).await?;
    let pr_store: Arc<dyn db::traits::PrChannelMessageStore> =
        Arc::new(PostgresPrChannelMessageStore::new(pool.clone()));
    let sub_store: Arc<dyn db::traits::SubscriptionStore> =
        Arc::new(PostgresSubscriptionStore::new(pool.clone()));
    let user_store: Arc<dyn db::traits::UserLinkStore> = Arc::new(PgPoolUserLinkStore::new(pool));

    let app_state = AppState {
        pr_store: Arc::clone(&pr_store),
        sub_store: Arc::clone(&sub_store),
        user_store: Arc::clone(&user_store),
        github: Arc::clone(&github),
        local_dev: config.local_dev,
        public_domain: config.public_domain.clone(),
        webhook_secret: config.github_webhook_secret.clone(),
    };

    let (mut discord_client, http) = discord::client::build(&config.discord_token, app_state).await;

    let webhook_state = WebhookState {
        secret: config.github_webhook_secret,
        http,
        github: Arc::clone(&github),
        user_store: Arc::clone(&user_store),
        pr_store: Arc::clone(&pr_store),
        sub_store: Arc::clone(&sub_store),
    };

    let http_task = tokio::spawn(serve_http(config.port, webhook_state));
    let discord_task = tokio::spawn(async move {
        discord_client
            .start()
            .await
            .expect("Discord client crashed");
    });

    tokio::select! {
        _ = http_task    => tracing::error!("HTTP server exited unexpectedly"),
        _ = discord_task => tracing::error!("Discord client exited unexpectedly"),
    }

    Ok(())
}

/// Initialize the global tracing subscriber.
///
/// Verbosity is controlled by `RUST_LOG` environment variable.
/// Defaults to `info` when the variable is absent.
fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
}

async fn serve_http(port: u16, state: WebhookState) {
    let app = axum::Router::new()
        .route("/healthz", axum::routing::get(healthz))
        .route(
            "/github/webhook",
            axum::routing::post(github::webhook::handle),
        )
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("HTTP server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind TCP listener");

    axum::serve(listener, app)
        .await
        .expect("HTTP server crashed");
}

/// Liveness probe for uptime monitors and Railway health checks.
async fn healthz() -> &'static str {
    "ok"
}
