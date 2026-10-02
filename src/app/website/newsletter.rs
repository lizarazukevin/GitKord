//! `POST /signup`: public newsletter signup.

use crate::models::newsletter_signup::{Email, NewsletterSignupStore};
use crate::AppError;
use axum::extract::{DefaultBodyLimit, State};
use axum::routing::post;
use axum::{Json, Router};
use http::StatusCode;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
struct NewsletterSignupPayload {
	email: String,
}

pub fn routes() -> Router<Arc<dyn NewsletterSignupStore>> {
	Router::new().route(
		"/newsletter/signup",
		post(newsletter_signup_handler).layer(DefaultBodyLimit::max(1024)),
	)
}

async fn newsletter_signup_handler(
	State(store): State<Arc<dyn NewsletterSignupStore>>,
	Json(payload): Json<NewsletterSignupPayload>,
) -> Result<StatusCode, AppError> {
	let email = Email::parse(&payload.email)?;
	store.upsert(&email).await?;
	Ok(StatusCode::OK)
}
