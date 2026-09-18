//! `lapin`-backed (AMQP 0-9-1 / `RabbitMQ`) implementation of the
//! [`crate::broker`] traits.

pub mod consumer;
pub mod publisher;

pub use consumer::RabbitMqConsumer;
pub use publisher::RabbitMqPublisher;

use crate::broker::BrokerError;
use crate::APP_NAME;
use lapin::{Connection, ConnectionProperties};

/// A connection to the broker, shared to build both publishers and
/// consumers. Thin wrapper so callers depend on this type rather than
/// `lapin::Connection` directly.
pub struct RabbitMqConnection {
	inner: Connection,
}

impl RabbitMqConnection {
	/// Connect to `uri` using `lapin`'s default (Tokio) runtime.
	///
	/// # Errors
	///
	/// Returns [`BrokerError::Connection`] if the broker can't be reached
	/// or the AMQP handshake fails.
	pub async fn connect(uri: &str) -> Result<Self, BrokerError> {
		let options = ConnectionProperties::default().with_connection_name(APP_NAME.into());

		let inner = Connection::connect(uri, options)
			.await
			.map_err(|e| BrokerError::Connection(e.to_string()))?;

		Ok(Self { inner })
	}

	/// Open a new AMQP channel on this connection.
	///
	/// # Errors
	///
	/// Returns [`BrokerError::Connection`] if the channel can't be opened
	/// (e.g. the connection has been closed).
	pub async fn create_channel(&self) -> Result<lapin::Channel, BrokerError> {
		self.inner
			.create_channel()
			.await
			.map_err(|e| BrokerError::Connection(e.to_string()))
	}
}
