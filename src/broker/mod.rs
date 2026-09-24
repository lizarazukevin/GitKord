//! Message broker abstraction layer.

pub mod error;
pub mod rabbitmq;

pub use error::BrokerError;
use std::collections::HashMap;

use async_trait::async_trait;
use std::future::Future;
use std::pin::Pin;

/// The topic exchange every `GitHub` webhook event is published to.
pub const GITHUB_EVENTS_EXCHANGE: &str = "gitkord.github.events";

/// The queue this consumer reads from.
/// Synced with `definitions.json`'s `github.events` queue.
pub const GITHUB_EVENTS_QUEUE: &str = "github.events";

/// Routing key bound to the terminal dead-letter queue.
pub const DLQ_ROUTING_KEY: &str = "dlq";

/// Routing key for retry-tier queues.
pub const REQUEUE_ROUTING_KEY: &str = "github.requeue";

/// Backoff tiers for failed deliveries, escalating in exponential retries
/// before ending up in the DLQ. Order matters for this sequence of routing keys.
pub const RETRY_TIERS: [&str; 3] = ["retry.30s", "retry.2m", "retry.8m"];

/// Header carrying how many times a message has been attempted.
pub const RETRY_ATTEMPT_HEADER: &str = "x-gitkord-attempt";

/// Header carrying the original routing key a message was published under.
pub const ORIGINAL_ROUTING_KEY_HEADER: &str = "x-gitkord-routing-key";

/// A message pulled off a queue.
#[derive(Debug, Clone)]
pub struct BrokerMessage {
	/// Routing key / topic the message was published under.
	pub routing_key: String,
	/// Raw message body, exactly as published.
	pub payload: Vec<u8>,
	/// Producer-assigned identifier carried alongside the message, used
	/// downstream for idempotency and log correlation, `None` when unset.
	/// (e.g. `GitHub` webhooks use `X-GitHub-Delivery` header value)
	pub delivery_id: Option<String>,
	/// Arbitrary, broker-agnostic string headers carried with the message.
	pub headers: HashMap<String, String>,
}

/// Message paths after consumer processes the delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsumeOutcome {
	/// Processed successfully; the broker may remove the message.
	Ack,
	#[allow(dead_code)]
	/// Transient failure; the broker should retry per its own policy.
	Retry,
	/// Non-retryable failure (e.g. malformed payload); route straight to
	/// a dead-letter queue without retrying.
	Reject,
}

/// A future resolving to a [`ConsumeOutcome`], boxed so it can cross the
/// trait-object boundary in [`ConsumeHandler`].
pub type ConsumeFuture = Pin<Box<dyn Future<Output = ConsumeOutcome> + Send>>;

/// Callback invoked once per message received by a [`MessageConsumer`].
pub type ConsumeHandler = std::sync::Arc<dyn Fn(BrokerMessage) -> ConsumeFuture + Send + Sync>;

/// Publishes messages onto the broker.
#[async_trait]
pub trait MessagePublisher: Send + Sync {
	/// Publish `payload` under `routing_key`, tagged with `delivery_id`.
	///
	/// # Errors
	///
	/// Returns [`BrokerError`] if the broker connection is unavailable, the
	/// publish is rejected, or the broker fails to confirm the publish.
	async fn publish(
		&self,
		routing_key: &str,
		delivery_id: &str,
		headers: &HashMap<String, String>,
		payload: &[u8],
	) -> Result<(), BrokerError>;
}

/// Consumes messages from a named queue.
#[async_trait]
pub trait MessageConsumer: Send + Sync {
	/// Run `handler` for every message on `queue` until the consumer is
	/// canceled or the underlying connection is lost. Implementations own
	/// their own reconnect policy.
	///
	/// # Errors
	///
	/// Returns [`BrokerError`] if the queue can't be consumed from, or if
	/// the underlying connection is lost and can't be recovered.
	async fn run(&self, queue: &str, handler: ConsumeHandler) -> Result<(), BrokerError>;
}
