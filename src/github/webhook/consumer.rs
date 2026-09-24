//! Runs the queue consumer: pulls `GitHub` webhook deliveries off the
//! broker and dispatches them to [`WebhookRouter::dispatch`].

use crate::broker::{
	BrokerMessage, ConsumeHandler, ConsumeOutcome, MessageConsumer, MessagePublisher,
	DLQ_ROUTING_KEY, GITHUB_EVENTS_QUEUE, ORIGINAL_ROUTING_KEY_HEADER, REQUEUE_ROUTING_KEY,
	RETRY_ATTEMPT_HEADER, RETRY_TIERS,
};
use crate::error::AppError;
use crate::github::webhook::events::models::GitHubEvent;
use crate::github::webhook::router::WebhookRouter;
use axum::body::Bytes;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::error;

/// Retry routing for a failed message.
enum EscalationTarget {
	/// Retry-tier queue with specified TTL.
	Retry(&'static str),
	/// Retries exhausted, goes to DLQ.
	DeadLetter,
}

/// Runs until the broker connection is lost or the consumer is canceled.
///
/// # Errors
///
/// Returns [`AppError::Broker`] if the consumer can't start, or the
/// underlying connection is lost and isn't recovered by the broker layer.
pub async fn run_queue_consumer(
	consumer: Arc<dyn MessageConsumer>,
	publisher: Arc<dyn MessagePublisher>,
	webhook_router: Arc<WebhookRouter>,
) -> Result<(), AppError> {
	let handler: ConsumeHandler = Arc::new(move |message: BrokerMessage| {
		let publisher = Arc::clone(&publisher);
		let webhook_router = Arc::clone(&webhook_router);
		Box::pin(async move { process_message(&publisher, &webhook_router, message).await })
	});

	consumer
		.run(GITHUB_EVENTS_QUEUE, handler)
		.await
		.map_err(AppError::from)
}

/// Extracts the event type and raw payload from the consumed message before
/// delivering the message to the webhook router marking it as ACK/NACK.
async fn process_message(
	publisher: &Arc<dyn MessagePublisher>,
	webhook_router: &WebhookRouter,
	message: BrokerMessage,
) -> ConsumeOutcome {
	let Some(delivery_id) = message.delivery_id.clone() else {
		error!(
			routing_key = %message.routing_key,
			"queued message has no delivery id, dropping message"
		);
		return ConsumeOutcome::Reject;
	};

	let event_routing_key = resolve_event_routing_key(&message).to_owned();

	let Some(event_type_str) = event_routing_key.strip_prefix("github.") else {
		error!(
			routing_key = %event_routing_key,
			delivery_id = %delivery_id,
			"queued message with an unrecognized routing key, dead-lettering"
		);
		return ConsumeOutcome::Reject;
	};

	// `github.requeue` is not a real event type, reaching this means the retried
	// message lost its `ORIGINAL_ROUTING_KEY_HEADER`.
	if event_routing_key == REQUEUE_ROUTING_KEY {
		error!(
			delivery_id = %delivery_id,
			"retried message lost its original-routing-key header, dead-lettering"
		);
		return ConsumeOutcome::Reject;
	}

	let event_type = GitHubEvent::from(event_type_str);
	let payload = Bytes::from(message.payload.clone());

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
			escalate(
				publisher.as_ref(),
				&message,
				&delivery_id,
				&event_routing_key,
				&e,
			)
			.await
		}
	}
}

/// Event type a message represents (e.g. `pull_request`).
/// This is read from the [`ORIGINAL_ROUTING_KEY_HEADER`], falls
/// back to the raw AMQP routing key as a defensive default.
fn resolve_event_routing_key(message: &BrokerMessage) -> &str {
	message
		.headers
		.get(ORIGINAL_ROUTING_KEY_HEADER)
		.map_or(message.routing_key.as_str(), String::as_str)
}

/// How many times this message has already been attempted to process.
/// Defaults to `1` if header is missing since it includes the current attempt.
fn current_attempt(message: &BrokerMessage) -> u32 {
	message
		.headers
		.get(RETRY_ATTEMPT_HEADER)
		.and_then(|value| value.parse::<u32>().ok())
		.unwrap_or(1)
}

/// Publishes a copy of `message` into the next retry tier, carrying
/// the original routing key and incremented attempt count. Acknowledges
/// the original off the primary queue.
async fn escalate(
	publisher: &dyn MessagePublisher,
	message: &BrokerMessage,
	delivery_id: &str,
	event_routing_key: &str,
	failure: &AppError,
) -> ConsumeOutcome {
	let failed_attempt = current_attempt(message);
	let next_attempt = failed_attempt.saturating_add(1);

	let (next_routing_key, target_description) = match next_escalation_target(failed_attempt) {
		EscalationTarget::Retry(tier) => (tier, "REQUEUE"),
		EscalationTarget::DeadLetter => (DLQ_ROUTING_KEY, "DLQ"),
	};

	let mut headers = HashMap::with_capacity(2);
	headers.insert(RETRY_ATTEMPT_HEADER.to_owned(), next_attempt.to_string());
	headers.insert(
		ORIGINAL_ROUTING_KEY_HEADER.to_owned(),
		event_routing_key.to_owned(),
	);

	match publisher
		.publish(next_routing_key, delivery_id, &headers, &message.payload)
		.await
	{
		Ok(()) => {
			error!(
				error = %failure,
				delivery_id,
				attempt = failed_attempt,
				routing_key = %next_routing_key,
				"failed to process queued webhook event, escalated to {target_description}"
			);
		}
		Err(publish_err) => {
			error!(
				original_error = %failure,
				publish_error = %publish_err,
				delivery_id,
				"failed to process queued webhook event AND failed to escalate, dropping because no DLQ copy exists for this message"
			);
		}
	}

	ConsumeOutcome::Ack
}

fn next_escalation_target(failed_attempt: u32) -> EscalationTarget {
	let tier_index = failed_attempt.saturating_sub(1);
	usize::try_from(tier_index)
		.ok()
		.and_then(|index| RETRY_TIERS.get(index))
		.map_or(EscalationTarget::DeadLetter, |tier| {
			EscalationTarget::Retry(tier)
		})
}
