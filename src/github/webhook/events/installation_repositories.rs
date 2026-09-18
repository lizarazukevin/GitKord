//! `installation_repositories` webhook event handler.

use crate::app::observability::{record_context_on_current_span, LogContext};
use crate::error::AppError;
use crate::github::webhook::events::models::{GitHubEvent, InstallationInfo, InstallationRepo};
use crate::github::webhook::router::WebhookEventHandler;
use crate::service::github::installation_repositories::{
	InstallationRepositoriesRequest, InstallationRepositoriesService,
};
use async_trait::async_trait;
use axum::body::Bytes;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct InstallationRepositoriesPayload {
	pub action: String,
	pub installation: InstallationInfo,
	#[allow(dead_code)]
	pub repositories_added: Vec<InstallationRepo>,
	pub repositories_removed: Vec<InstallationRepo>,
	#[allow(dead_code)]
	pub repository_selection: String,
}

pub struct InstallationRepositoriesEventHandler {
	service: Arc<InstallationRepositoriesService>,
}

impl InstallationRepositoriesEventHandler {
	pub const fn new(service: Arc<InstallationRepositoriesService>) -> Self {
		Self { service }
	}
}

#[async_trait]
impl WebhookEventHandler for InstallationRepositoriesEventHandler {
	fn event_type(&self) -> GitHubEvent {
		GitHubEvent::InstallationRepositories
	}

	async fn execute(&self, body: Bytes) -> Result<(), AppError> {
		let payload: InstallationRepositoriesPayload = serde_json::from_slice(&body)?;

		record_context_on_current_span(&LogContext {
			installation_id: Some(payload.installation.id.0),
			github_user: Some(payload.installation.account.login.clone()),
			..LogContext::default()
		});

		let req = InstallationRepositoriesRequest::from_payload(payload);
		self.service.handle(req).await
	}
}
