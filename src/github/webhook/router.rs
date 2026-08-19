//! Dispatches incoming `GitHub` webhook requests to the handler
//! registered for that event type.

use crate::app::observability::{observe, EventKind, LogContext, MetricsRecorder};
use crate::error::AppError;
use crate::github::webhook::events::installation::InstallationEventHandler;
use crate::github::webhook::events::issue_comment::IssueCommentEventHandler;
use crate::github::webhook::events::models::GitHubEvent;
use crate::github::webhook::events::pull_request::PullRequestEventHandler;
use crate::github::webhook::events::review::ReviewEventHandler;
use crate::github::webhook::signature::WebhookVerifier;
use crate::service::github::installation::InstallationService;
use crate::service::github::issue_comment::IssueCommentService;
use crate::service::github::pull_request::PullRequestService;
use crate::service::github::review::ReviewService;
use async_trait::async_trait;
use axum::body::Bytes;
use axum::response::{IntoResponse, Response};
use http::{HeaderMap, StatusCode};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

/// A handler for one `GitHub` webhook event type. Deserializes the payload,
/// invokes its service, and returns an HTTP response (or an [`AppError`]).
#[async_trait]
pub trait WebhookEventHandler: Send + Sync {
	/// The event type this handler is registered for.
	fn event_type(&self) -> GitHubEvent;
	/// Process a raw webhook body and produce the HTTP response.
	async fn execute(&self, body: Bytes) -> Result<Response, AppError>;
}

/// Verifies signatures and dispatches each event to its registered handler.
pub struct WebhookRouter {
	verifier: WebhookVerifier,
	handlers: HashMap<GitHubEvent, Arc<dyn WebhookEventHandler>>,
	recorder: Arc<dyn MetricsRecorder>,
}

impl WebhookRouter {
	pub fn new(
		secret: String,
		pull_request_service: Arc<PullRequestService>,
		review_service: Arc<ReviewService>,
		issue_comment_service: Arc<IssueCommentService>,
		installation_service: Arc<InstallationService>,
		recorder: Arc<dyn MetricsRecorder>,
	) -> Self {
		let pr: Arc<dyn WebhookEventHandler> =
			Arc::new(PullRequestEventHandler::new(pull_request_service));
		let review: Arc<dyn WebhookEventHandler> =
			Arc::new(ReviewEventHandler::new(review_service));
		let issue: Arc<dyn WebhookEventHandler> =
			Arc::new(IssueCommentEventHandler::new(issue_comment_service));
		let installation: Arc<dyn WebhookEventHandler> =
			Arc::new(InstallationEventHandler::new(installation_service));

		let handlers = [pr, review, issue, installation]
			.into_iter()
			.map(|h| (h.event_type(), h))
			.collect();

		Self {
			verifier: WebhookVerifier::new(secret),
			handlers,
			recorder,
		}
	}

	/// Verify the signature, short-circuit pings, then dispatch to a handler.
	pub async fn route(self: Arc<Self>, headers: HeaderMap, body: Bytes) -> Response {
		if self.verifier.verify(&headers, &body).is_err() {
			return StatusCode::UNAUTHORIZED.into_response();
		}

		let event_type = Self::resolve_event_type(&headers);

		if event_type == GitHubEvent::Ping {
			info!("GitHub ping received, webhook is connected");
			return StatusCode::OK.into_response();
		}

		self.dispatch(event_type, body).await
	}

	/// Reads the `X-GitHub-Event` header into a [`GitHubEvent`], treating
	/// a missing header the same as an event type we don't recognize.
	fn resolve_event_type(headers: &HeaderMap) -> GitHubEvent {
		headers
			.get("x-github-event")
			.and_then(|v| v.to_str().ok())
			.map_or_else(
				|| GitHubEvent::Unknown("missing header".into()),
				GitHubEvent::from,
			)
	}

	/// Runs the handler registered for `event_type`, or logs and no-ops for unhandled events.
	async fn dispatch(&self, event_type: GitHubEvent, body: Bytes) -> Response {
		let Some(handler) = self.handlers.get(&event_type) else {
			info!(?event_type, "unhandled event type");
			return StatusCode::OK.into_response();
		};

		observe(
			EventKind::Webhook,
			event_type.as_str(),
			&LogContext::default(),
			handler.execute(body),
			self.recorder.as_ref(),
		)
		.await
		.unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
	}
}
