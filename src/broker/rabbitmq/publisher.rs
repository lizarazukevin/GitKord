//! `lapin`-backed implementation of [`MessagePublisher`].

use crate::broker::{BrokerError, MessagePublisher};
use async_trait::async_trait;
use lapin::options::{BasicPublishOptions, ConfirmSelectOptions};
use lapin::Confirmation;
use lapin::{BasicProperties, Channel};

/// The exchange every `GitHub` webhook event is published to.
pub const GITHUB_EVENTS_EXCHANGE: &str = "gitkord.github.events";

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
		payload: &[u8],
	) -> Result<(), BrokerError> {
		let properties = BasicProperties::default()
			.with_delivery_mode(2)
			.with_content_type("application/json".into())
			.with_message_id(delivery_id.into());

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
