use anyhow::Context;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::app::shutdown::shutdown_signal;
use crate::error::AppError;
use crate::github::webhook::router::WebhookRouter;
use axum::routing::{get, post};
use axum::{serve, Router};
use tokio::net::TcpListener;
use tracing::info;

/// Serve the webhook + health endpoints until graceful shutdown.
pub async fn serve_http(port: u16, router: Arc<WebhookRouter>) -> Result<(), AppError> {
    let app = Router::new().route("/healthz", get(healthz)).route(
        "/github/webhook",
        post({
            let router = router.clone();
            move |headers, body| router.route(headers, body)
        }),
    );

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
