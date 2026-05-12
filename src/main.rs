#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]
#![allow(clippy::multiple_crate_versions)] // transitive deps — not in our control

//! `DiGiBot` entrypoint.
//!
//! Two long-running tasks share the process:
//! - An Axum HTTP server — receives GitHub webhook payloads.
//! - A Serenity Discord client — maintains the gateway connection,
//!   handles slash commands and button interactions.
//!
//! Both are spawned as Tokio tasks so neither blocks the other.
//! If either task exits unexpectedly the process exits with a non-zero code
//! so Railway (or any supervisor) knows to restart it.

mod config;
mod discord;
mod error;
mod github;

use crate::github::webhook::WebhookState;
use anyhow::Result;
use serenity::all::ChannelId;
use std::net::SocketAddr;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let config = config::Config::from_env()?;
    info!("DiGiBot starting — watching {}", config.github_repo);

    // Builds the Discord client and extract the HTTP handle before client
    // is moved into its task, HTTP is Arc-backed so cloning is cheap.
    let (mut discord_client, http) = discord::bot::build(&config.discord_token).await;

    let webhook_state = WebhookState {
        secret: config.github_webhook_secret,
        http,
        channel_id: ChannelId::new(config.discord_channel_id),
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

// — Initializers —————————————————————————————————————————————————

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

/// Bind a TCP listener and start the Axum HTTP server.
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

// — HTTP Handlers ————————————————————————————————————————————————

/// Liveness probe, returns `200 OK` for uptime monitors and Railway health checks.
async fn healthz() -> &'static str {
    "ok"
}
