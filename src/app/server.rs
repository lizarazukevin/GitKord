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

/// Serve the webhook, health, and metrics endpoints until graceful shutdown.
pub async fn serve_http(
	port: u16,
	router: Arc<WebhookRouter>,
	renderer: Arc<dyn MetricsRenderer>,
) -> Result<(), AppError> {
	let app = Router::new()
		.route("/metrics", get(metrics))
		.route("/healthz", get(healthz))
		.route(
			"/github/webhook",
			post({
				let router = router.clone();
				move |headers, body| router.route(headers, body)
			}),
		)
		.with_state(renderer);

	let addr = SocketAddr::from(([0, 0, 0, 0], port));
	info!("HTTP server listening on {addr}");

	let listener = TcpListener::bind(addr)
		.await
		.with_context(|| format!("failed to bind HTTP listener on {addr}"))?;
	serve(listener, app)
		.with_graceful_shutdown(shutdown_signal())
		.await
		.context("HTTP server error")?;

	Ok(())
}

async fn healthz() -> &'static str {
	"ok"
}

/// Renders the current metrics snapshot for Prometheus scraping.
async fn metrics(State(renderer): State<Arc<dyn MetricsRenderer>>) -> String {
	renderer.as_ref().render()
}
