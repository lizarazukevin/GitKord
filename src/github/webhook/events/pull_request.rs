//! `pull_request` webhook event handler.

use crate::error::AppError;
use crate::github::webhook::events::models::{
    GitHubEvent, GitHubUserInfo, PullRequestInfo, RepositoryInfo,
};
use crate::github::webhook::router::WebhookEventHandler;
use crate::service::github::pull_request::{PullRequestRequest, PullRequestService};
use async_trait::async_trait;
use axum::body::Bytes;
use axum::response::{IntoResponse, Response};
use http::StatusCode;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct PullRequestPayload {
    pub action: String,
    pub pull_request: PullRequestInfo,
    pub repository: RepositoryInfo,
    pub sender: GitHubUserInfo,
}

pub struct PullRequestEventHandler {
    service: Arc<PullRequestService>,
}

impl PullRequestEventHandler {
    pub fn new(service: Arc<PullRequestService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl WebhookEventHandler for PullRequestEventHandler {
    fn event_type(&self) -> GitHubEvent {
        GitHubEvent::PullRequest
    }

    async fn execute(&self, body: Bytes) -> Result<Response, AppError> {
        let payload: PullRequestPayload =
            serde_json::from_slice(&body).map_err(anyhow::Error::from)?;
        let req = PullRequestRequest::from_payload(payload);
        self.service.handle(req).await?;
        Ok(StatusCode::OK.into_response())
    }
}
