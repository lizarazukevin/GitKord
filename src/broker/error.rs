//! Error type for the broker abstraction.

/// An error from the message-broker layer, broker-implementation-agnostic.
#[derive(Debug, thiserror::Error)]
pub enum BrokerError {
	/// Couldn't establish or re-establish a connection to the broker.
	#[error("failed to connect to broker: {0}")]
	Connection(String),

	/// A publish attempt failed, was rejected, or wasn't confirmed.
	#[error("failed to publish message: {0}")]
	Publish(String),

	/// Declaring an exchange, queue, or binding failed.
	#[error("failed to declare broker topology: {0}")]
	Topology(String),

	/// A consumer failed to start, or terminated unexpectedly.
	#[error("consumer error: {0}")]
	Consumer(String),
}
