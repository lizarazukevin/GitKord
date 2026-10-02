//! Admin newsletter endpoints: list signups and opt emails out.

use crate::models::newsletter_signup::NewsletterSignup;
use crate::service::newsletter::{NewsletterEmailRequest, NewsletterService};
use crate::AppError;
use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use http::StatusCode;
use serde::Deserialize;
use std::sync::Arc;

type Service = Arc<NewsletterService>;

#[derive(Deserialize)]
struct SignupsParams {
	max_results: Option<u32>,
	next_token: Option<i64>,
	is_subscribed: Option<bool>,
}

pub fn routes() -> Router<Service> {
	Router::new()
		.route("/newsletter/signups", get(signups_handler))
		.route("/newsletter/unsubscribe", post(unsubscribe_handler))
}

async fn signups_handler(
	State(service): State<Service>,
	Query(params): Query<SignupsParams>,
) -> Result<Json<Vec<NewsletterSignup>>, AppError> {
	let rows = service
		.list(params.max_results, params.next_token, params.is_subscribed)
		.await?;
	Ok(Json(rows))
}

async fn unsubscribe_handler(
	State(service): State<Service>,
	Json(payload): Json<NewsletterEmailRequest>,
) -> Result<StatusCode, AppError> {
	service.unsubscribe(&payload.email).await?;
	Ok(StatusCode::NO_CONTENT)
}
