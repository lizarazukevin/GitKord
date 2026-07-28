//! `pull_request_review` webhook event handler.

use crate::error::AppError;
use crate::github::webhook::events::models::{
	GitHubEvent, PullRequestInfo, RepositoryInfo, ReviewInfo,
};
use crate::github::webhook::router::WebhookEventHandler;
use crate::service::github::review::{ReviewRequest, ReviewService};
use async_trait::async_trait;
use axum::body::Bytes;
use axum::response::{IntoResponse, Response};
use http::StatusCode;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct PullRequestReviewPayload {
	pub action: String,
	pub review: ReviewInfo,
	pub pull_request: PullRequestInfo,
	pub repository: RepositoryInfo,
}

pub struct ReviewEventHandler {
	service: Arc<ReviewService>,
}

impl ReviewEventHandler {
	pub fn new(service: Arc<ReviewService>) -> Self {
		Self { service }
	}
}

#[async_trait]
impl WebhookEventHandler for ReviewEventHandler {
	fn event_type(&self) -> GitHubEvent {
		GitHubEvent::PullRequestReview
	}

	async fn execute(&self, body: Bytes) -> Result<Response, AppError> {
		let payload: PullRequestReviewPayload =
			serde_json::from_slice(&body).map_err(anyhow::Error::from)?;
		let req = ReviewRequest::from_payload(payload);
		self.service.handle(req).await?;
		Ok(StatusCode::OK.into_response())
	}
}
