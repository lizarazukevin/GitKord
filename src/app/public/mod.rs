//! Routing for public endpoints.

use crate::service::newsletter::NewsletterService;
use axum::Router;
use std::sync::Arc;

pub mod newsletter;

pub fn router(service: Arc<NewsletterService>) -> Router {
	Router::new()
		.merge(newsletter::routes())
		.with_state(service)
}
