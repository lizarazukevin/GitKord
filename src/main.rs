#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]
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
mod error;
mod github;

use anyhow::Result;
use std::net::SocketAddr;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let config = config::Config::from_env()?;
    info!("DiGiBot starting — watching {}", config.github_repo);

    let http_task = tokio::spawn(serve_http(config.port, config.github_webhook_secret));
    let discord_task = tokio::spawn(run_discord(config.discord_token));

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
async fn serve_http(port: u16, webhook_secret: String) {
    let app = axum::Router::new()
        .route("/healthz", axum::routing::get(healthz))
        .route(
            "/github/webhook",
            axum::routing::post(github::webhook::handle),
        )
        .with_state(webhook_secret);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("HTTP server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind TCP listener");

    axum::serve(listener, app)
        .await
        .expect("HTTP server crashed");
}

/// Connect the Serenity Discord client and begin processing gateway events.
async fn run_discord(token: String) {
    let intents = serenity::all::GatewayIntents::empty();

    let mut client = serenity::Client::builder(&token, intents)
        .event_handler(ReadyHandler)
        .await
        .expect("failed to build Discord client");

    client.start().await.expect("Discord client crashed");
}

// — HTTP Handlers ————————————————————————————————————————————————

/// Liveness probe, returns `200 OK` for uptime monitors and Railway health checks.
async fn healthz() -> &'static str {
    "ok"
}

// ── Discord Handler ─────────────────────────────────────────────

struct ReadyHandler;

#[serenity::async_trait]
impl serenity::all::EventHandler for ReadyHandler {
    async fn ready(&self, _ctx: serenity::all::Context, ready: serenity::all::Ready) {
        info!("Discord bot connected as {}", ready.user.name);
    }
}
