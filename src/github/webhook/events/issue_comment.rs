//! `issue_comment` webhook event handler.

use crate::error::AppError;
use crate::github::webhook::events::models::{GitHubEvent, IssueInfo, RepositoryInfo};
use crate::github::webhook::router::WebhookEventHandler;
use crate::service::github::issue_comment::{IssueCommentRequest, IssueCommentService};
use async_trait::async_trait;
use axum::body::Bytes;
use axum::response::{IntoResponse, Response};
use http::StatusCode;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct IssueCommentPayload {
    pub action: String,
    pub issue: IssueInfo,
    pub repository: RepositoryInfo,
}

pub struct IssueCommentEventHandler {
    service: Arc<IssueCommentService>,
}

impl IssueCommentEventHandler {
    pub fn new(service: Arc<IssueCommentService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl WebhookEventHandler for IssueCommentEventHandler {
    fn event_type(&self) -> GitHubEvent {
        GitHubEvent::IssueComment
    }

    async fn execute(&self, body: Bytes) -> Result<Response, AppError> {
        let payload: IssueCommentPayload =
            serde_json::from_slice(&body).map_err(anyhow::Error::from)?;
        let req = IssueCommentRequest::from_payload(payload);
        self.service.handle(req).await?;
        Ok(StatusCode::OK.into_response())
    }
}
