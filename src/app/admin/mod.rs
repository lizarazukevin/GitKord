//! Routing for protected endpoints.

pub mod newsletter;

use crate::service::newsletter::NewsletterService;
use axum::extract::{Request, State};
use axum::middleware::{from_fn_with_state, Next};
use axum::response::Response;
use axum::Router;
use http::header::AUTHORIZATION;
use http::StatusCode;
use std::sync::Arc;
use subtle::ConstantTimeEq;

pub fn router(service: Arc<NewsletterService>, token: Arc<str>) -> Router {
	Router::new().nest(
		"/admin",
		Router::new()
			.merge(newsletter::routes())
			.route_layer(from_fn_with_state(token, require_admin))
			.with_state(service),
	)
}

async fn require_admin(
	State(token): State<Arc<str>>,
	req: Request,
	next: Next,
) -> Result<Response, StatusCode> {
	if token.is_empty() {
		return Err(StatusCode::UNAUTHORIZED);
	}

	let supplied = req
		.headers()
		.get(AUTHORIZATION)
		.and_then(|v| v.to_str().ok())
		.and_then(|v| v.strip_prefix("Bearer "));

	let authorized = supplied.is_some_and(|t| t.as_bytes().ct_eq(token.as_ref().as_bytes()).into());

	if authorized {
		Ok(next.run(req).await)
	} else {
		Err(StatusCode::UNAUTHORIZED)
	}
}
