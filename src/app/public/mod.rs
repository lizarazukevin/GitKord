//! Routing for public endpoints.

use crate::service::business_metrics::BusinessMetricsService;
use crate::service::newsletter::NewsletterService;
use axum::Router;
use http::header::CONTENT_TYPE;
use http::{HeaderValue, Method};
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, CorsLayer};

pub mod metrics;
pub mod newsletter;

/// Origins allowed to call the public endpoints from a browser.
const ALLOWED_ORIGINS: [&str; 2] = ["https://gitkord.com", "https://www.gitkord.com"];

pub fn router(service: Arc<NewsletterService>, metrics: Arc<BusinessMetricsService>) -> Router {
	Router::new()
		.merge(newsletter::routes().with_state(service))
		.merge(metrics::routes().with_state(metrics))
		.layer(
			CorsLayer::new()
				.allow_origin(AllowOrigin::list(
					ALLOWED_ORIGINS.map(HeaderValue::from_static),
				))
				.allow_methods([Method::GET, Method::POST])
				.allow_headers([CONTENT_TYPE]),
		)
}
