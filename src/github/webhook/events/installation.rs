//! `GitHub` app installation webhook handler.

use crate::error::AppError;
use crate::github::webhook::events::models::{GitHubEvent, InstallationInfo};
use crate::github::webhook::router::WebhookEventHandler;
use crate::service::github::installation::{InstallationRequest, InstallationService};
use async_trait::async_trait;
use axum::body::Bytes;
use axum::response::{IntoResponse, Response};
use http::StatusCode;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct InstallationPayload {
	pub action: String,
	pub installation: InstallationInfo,
	pub repositories: Vec<String>,
}

pub struct InstallationEventHandler {
	service: Arc<InstallationService>,
}

impl InstallationEventHandler {
	pub const fn new(service: Arc<InstallationService>) -> Self {
		Self { service }
	}
}

#[async_trait]
impl WebhookEventHandler for InstallationEventHandler {
	fn event_type(&self) -> GitHubEvent {
		GitHubEvent::Installation
	}

	async fn execute(&self, body: Bytes) -> Result<Response, AppError> {
		let payload: InstallationPayload =
			serde_json::from_slice(&body).map_err(anyhow::Error::from)?;
		let req = InstallationRequest::from_payload(payload);
		self.service.handle(req).await?;
		Ok(StatusCode::OK.into_response())
	}
}
