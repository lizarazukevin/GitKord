//! Public endpoints for newsletter activity.

use crate::service::newsletter::{NewsletterEmailRequest, NewsletterService};
use crate::AppError;
use axum::extract::{DefaultBodyLimit, State};
use axum::routing::post;
use axum::{Json, Router};
use http::StatusCode;
use std::sync::Arc;

pub fn routes() -> Router<Arc<NewsletterService>> {
	Router::new().route(
		"/newsletter/signup",
		post(newsletter_signup_handler).layer(DefaultBodyLimit::max(1024)),
	)
}

async fn newsletter_signup_handler(
	State(service): State<Arc<NewsletterService>>,
	Json(payload): Json<NewsletterEmailRequest>,
) -> Result<StatusCode, AppError> {
	service.subscribe(&payload.email).await?;
	Ok(StatusCode::OK)
}
