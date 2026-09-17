//! Verifies incoming `GitHub` webhook requests and publishes them to the
//! broker. Also dispatches queued deliveries to the proper registered handler
//! for that event type.

use crate::app::observability::{observe, EventKind, LogContext, MetricsRecorder};
use crate::broker::MessagePublisher;
use crate::error::AppError;
use crate::github::webhook::events::models::GitHubEvent;
use crate::github::webhook::signature::WebhookVerifier;
use async_trait::async_trait;
use axum::body::Bytes;
use axum::response::{IntoResponse, Response};
use http::{HeaderMap, StatusCode};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info, warn};

/// A handler for one `GitHub` webhook event type. Deserializes the payload,
/// invokes its service, and returns an HTTP response (or an [`AppError`]).
#[async_trait]
pub trait WebhookEventHandler: Send + Sync {
	/// The event type this handler is registered for.
	fn event_type(&self) -> GitHubEvent;

	/// Deserializes the webhook `body` and runs service logic for event type.
	/// HTTP-agnostic, called exclusively from the queue consumer, not directly
	/// from an HTTP request.
	async fn execute(&self, body: Bytes) -> Result<(), AppError>;
}

/// Verifies signatures, publishes valid deliveries, and dispatches each
/// consumed event to its registered handler.
pub struct WebhookRouter {
	verifier: WebhookVerifier,
	handlers: HashMap<GitHubEvent, Arc<dyn WebhookEventHandler>>,
	recorder: Arc<dyn MetricsRecorder>,
	publisher: Arc<dyn MessagePublisher>,
}

impl WebhookRouter {
	pub fn new(
		secret: String,
		handlers: impl IntoIterator<Item = Arc<dyn WebhookEventHandler>>,
		recorder: Arc<dyn MetricsRecorder>,
		publisher: Arc<dyn MessagePublisher>,
	) -> Self {
		let handlers = handlers.into_iter().map(|h| (h.event_type(), h)).collect();

		Self {
			verifier: WebhookVerifier::new(secret),
			handlers,
			recorder,
			publisher,
		}
	}

	/// Verify the signature, short-circuit pings, then dispatch to a handler.
	pub async fn route(self: Arc<Self>, headers: HeaderMap, body: Bytes) -> Response {
		if self.verifier.verify(&headers, &body).is_err() {
			return StatusCode::UNAUTHORIZED.into_response();
		}

		let Some(event_type) = Self::resolve_event_type(&headers) else {
			warn!("webhook missing X-GitHub-Event header");
			return (StatusCode::BAD_REQUEST, "missing X-GitHub-Event header").into_response();
		};

		if event_type == GitHubEvent::Ping {
			info!("GitHub ping received, webhook is connected");
			return StatusCode::OK.into_response();
		}

		if !self.handlers.contains_key(&event_type) {
			info!(?event_type, "unhandled event type");
			return StatusCode::OK.into_response();
		}

		self.publish(&event_type, &headers, body).await
	}

	async fn publish(
		&self,
		event_type: &GitHubEvent,
		headers: &HeaderMap,
		body: Bytes,
	) -> Response {
		let Some(delivery_id) = Self::resolve_delivery_id(headers) else {
			warn!(?event_type, "webhook missing X-GitHub-Delivery header");
			return (StatusCode::BAD_REQUEST, "missing X-GitHub-Delivery header").into_response();
		};

		let routing_key = format!("github.{}", event_type.as_str());

		match self
			.publisher
			.publish(&routing_key, &delivery_id, &body)
			.await
		{
			Ok(()) => StatusCode::OK.into_response(),
			Err(e) => {
				error!(error = %e, ?event_type, "webhook publish error");
				StatusCode::INTERNAL_SERVER_ERROR.into_response()
			}
		}
	}

	/// Reads the `X-GitHub-Delivery`, `GitHub`'s own UUID that idempotency
	/// downstream keys off of.
	fn resolve_delivery_id(headers: &HeaderMap) -> Option<String> {
		headers
			.get("X-Github-Delivery")
			.and_then(|h| h.to_str().ok())
			.map(ToString::to_string)
	}

	/// Reads the `X-GitHub-Event` header into a [`GitHubEvent`], treating
	/// a missing header the same as an event type we don't recognize.
	fn resolve_event_type(headers: &HeaderMap) -> Option<GitHubEvent> {
		headers
			.get("X-Github-Event")
			.and_then(|v| v.to_str().ok())
			.map(GitHubEvent::from)
	}

	/// Consumer entry point. Runs the handler registered for `event_type` against
	/// a queued delivery.
	pub async fn dispatch(&self, event_type: GitHubEvent, body: Bytes) -> Result<(), AppError> {
		let Some(handler) = self.handlers.get(&event_type) else {
			return Err(AppError::UnroutableEvent(format!("{event_type:?}")));
		};

		observe(
			EventKind::Webhook,
			event_type.as_str(),
			&LogContext::default(),
			handler.execute(body),
			self.recorder.as_ref(),
		)
		.await
	}
}
