//! Message broker abstraction layer.

pub mod error;
pub mod rabbitmq;

pub use error::BrokerError;

use async_trait::async_trait;
use std::future::Future;
use std::pin::Pin;

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
}

/// Message paths after consumer processes the delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsumeOutcome {
	/// Processed successfully; the broker may remove the message.
	Ack,
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
