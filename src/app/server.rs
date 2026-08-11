//! HTTP server serving:
//! - webhook traffic from `GitHub`
//! - health endpoint for deployment monitoring
//! - metric endpoint for observability

use anyhow::Context;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::app::observability::renderer::MetricsRenderer;
use crate::app::shutdown::shutdown_signal;
use crate::error::AppError;
use crate::github::webhook::router::WebhookRouter;
use axum::extract::State;
use axum::routing::{get, post};
use axum::{serve, Router};
use tokio::net::TcpListener;
use tracing::info;

/// Serve the endpoints until graceful shutdown.
/// Metrics endpoint is restricted to internal network.
/// GitHub webhook is required to be public, secured by HMAC signatures.
/// Health endpoint is sanitized depending on the network accessed from.
pub async fn serve_http(
	port: u16,
	internal_port: u16,
	router: Arc<WebhookRouter>,
	renderer: Arc<dyn MetricsRenderer>,
) -> Result<(), AppError> {
	let public_app = Router::new().route("/healthz", get(healthz)).route(
		"/github/webhook",
		post({
			let router = router.clone();
			move |headers, body| router.route(headers, body)
		}),
	);

	let internal_app = Router::new()
		.route("/metrics", get(metrics))
		.route("healthz", get(healthz))
		.with_state(renderer);

	let public_listener = bind_listener(SocketAddr::from(([0, 0, 0, 0], port)), "public").await?;
	let internal_listener = bind_listener(
		SocketAddr::from(([127, 0, 0, 1], internal_port)),
		"internal",
	)
	.await?;

	let public_server =
		serve(public_listener, public_app).with_graceful_shutdown(shutdown_signal());
	let internal_server =
		serve(internal_listener, internal_app).with_graceful_shutdown(shutdown_signal());

	tokio::try_join!(
		async { public_server.await.context("public HTTP server error") },
		async { internal_server.await.context("internal HTTP server error") },
	)?;

	Ok(())
}

async fn bind_listener(addr: SocketAddr, label: &str) -> Result<TcpListener, AppError> {
	info!("{label} HTTP server listening on {addr}");
	TcpListener::bind(addr)
		.await
		.with_context(|| format!("failed to bind {label} HTTP listener on {addr}"))
		.map_err(AppError::from)
}

/// Sanitized health check for HTTP listener — confirms liveness only.
/// Add dependency checks for an internal version.
async fn healthz() -> &'static str {
	"ok"
}

/// Renders the current metrics snapshot for Prometheus scraping.
/// Only reachable on the internal listener.
async fn metrics(State(renderer): State<Arc<dyn MetricsRenderer>>) -> String {
	renderer.as_ref().render()
}
