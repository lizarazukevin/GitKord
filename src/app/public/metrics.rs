//! Public endpoints exposing cached business metrics to the public site.

use crate::service::business_metrics::{BusinessMetrics, BusinessMetricsService};
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use std::sync::Arc;

pub fn routes() -> Router<Arc<BusinessMetricsService>> {
	Router::new().route("/metrics/summary", get(summary_handler))
}

/// Serve the latest cached aggregate snapshot. No database I/O, the
/// `BusinessMetricsWorker` keeps this cache off the request path.
async fn summary_handler(
	State(service): State<Arc<BusinessMetricsService>>,
) -> Json<BusinessMetrics> {
	Json(service.snapshot().await)
}
