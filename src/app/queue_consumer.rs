//! Runs the queue consumer: pulls `GitHub` webhook deliveries off the
//! broker and dispatches them to [`WebhookRouter::dispatch`].

use crate::broker::{BrokerMessage, ConsumeHandler, ConsumeOutcome, MessageConsumer};
use crate::error::AppError;
use crate::github::webhook::events::models::GitHubEvent;
use crate::github::webhook::router::WebhookRouter;
use axum::body::Bytes;
use std::sync::Arc;
use tracing::error;

/// The queue this consumer reads from.
/// Synced with `definitions.json`'s `github.events` queue.
pub const GITHUB_EVENTS_QUEUE: &str = "github.events";

/// Runs until the broker connection is lost or the consumer is canceled.
///
/// # Errors
///
/// Returns [`AppError::Broker`] if the consumer can't start, or the
/// underlying connection is lost and isn't recovered by the broker layer.
pub async fn run_queue_consumer(
	consumer: Arc<dyn MessageConsumer>,
	webhook_router: Arc<WebhookRouter>,
) -> Result<(), AppError> {
	let handler: ConsumeHandler = Arc::new(move |message: BrokerMessage| {
		let webhook_router = Arc::clone(&webhook_router);
		Box::pin(async move { process_message(&webhook_router, message).await })
	});

	consumer
		.run(GITHUB_EVENTS_QUEUE, handler)
		.await
		.map_err(AppError::from)
}

/// Extracts the event type and raw payload from the consumed message before
/// delivering the message to the webhook router marking it as ACK/NACK.
async fn process_message(webhook_router: &WebhookRouter, message: BrokerMessage) -> ConsumeOutcome {
	let Some(delivery_id) = message.delivery_id.clone() else {
		error!(
			routing_key = %message.routing_key,
			"queued message has no delivery id, dropping message"
		);
		return ConsumeOutcome::Reject;
	};

	let Some(event_type_str) = message.routing_key.strip_prefix("github.") else {
		error!(
			routing_key = %message.routing_key,
			"queued message with an unrecognized routing key, dead-lettering"
		);
		return ConsumeOutcome::Reject;
	};

	let event_type = GitHubEvent::from(event_type_str);
	let payload = Bytes::from(message.payload);

	match webhook_router.dispatch(event_type, payload).await {
		Ok(()) => ConsumeOutcome::Ack,
		Err(AppError::UnroutableEvent(_) | AppError::Deserialization(_)) => {
			error!(
				delivery_id = %delivery_id,
				"dropping queued webhook event, not retryable"
			);
			ConsumeOutcome::Reject
		}
		Err(e) => {
			error!(
				error = %e,
				delivery_id = %delivery_id,
				"failed to process queued webhook event"
			);
			ConsumeOutcome::Retry
		}
	}
}
