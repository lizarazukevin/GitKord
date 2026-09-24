//! `lapin`-backed implementation of [`MessagePublisher`].

use crate::broker::{BrokerError, MessagePublisher};
use async_trait::async_trait;
use lapin::options::{BasicPublishOptions, ConfirmSelectOptions};
use lapin::types::{AMQPValue, FieldTable};
use lapin::Confirmation;
use lapin::{BasicProperties, Channel};
use std::collections::HashMap;

/// Publishes onto a single exchange with publisher confirms enabled.
/// Broker will return `Ok(())` once the broker has actually accepted and
/// persisted the message.
pub struct RabbitMqPublisher {
	channel: Channel,
	exchange: String,
}

impl RabbitMqPublisher {
	/// Wrap `channel`, enabling publisher confirms on it.
	///
	/// # Errors
	///
	/// Returns [`BrokerError::Topology`] if publisher confirms can't be
	/// enabled on the channel (e.g. it's already in transaction mode).
	pub async fn new(channel: Channel, exchange: String) -> Result<Self, BrokerError> {
		channel
			.confirm_select(ConfirmSelectOptions::default())
			.await
			.map_err(|e| BrokerError::Topology(e.to_string()))?;

		Ok(Self { channel, exchange })
	}
}

#[async_trait]
impl MessagePublisher for RabbitMqPublisher {
	async fn publish(
		&self,
		routing_key: &str,
		delivery_id: &str,
		headers: &HashMap<String, String>,
		payload: &[u8],
	) -> Result<(), BrokerError> {
		let properties = BasicProperties::default()
			.with_delivery_mode(2) // persistent option
			.with_content_type("application/json".into())
			.with_message_id(delivery_id.into())
			.with_headers(to_field_table(headers));

		let confirmation: Confirmation = self
			.channel
			.basic_publish(
				self.exchange.clone().into(),
				routing_key.into(),
				BasicPublishOptions::default(),
				payload,
				properties,
			)
			.await
			.map_err(|e| BrokerError::Publish(e.to_string()))?
			.await
			.map_err(|e| BrokerError::Publish(e.to_string()))?;

		if confirmation.is_nack() {
			return Err(BrokerError::Publish(
				"broker nacked the publish confirm".to_owned(),
			));
		}

		Ok(())
	}
}

/// Converts map of headers into an AMQP `FieldTable` as `LongString` values.
fn to_field_table(headers: &HashMap<String, String>) -> FieldTable {
	let mut table = FieldTable::default();
	for (key, value) in headers {
		table.insert(
			key.as_str().into(),
			AMQPValue::LongString(value.as_str().into()),
		);
	}
	table
}
