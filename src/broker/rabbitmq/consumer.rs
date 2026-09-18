//! `lapin`-backed implementation of [`MessageConsumer`].

use crate::broker::{BrokerError, BrokerMessage, ConsumeHandler, ConsumeOutcome, MessageConsumer};
use async_trait::async_trait;
use futures_lite::stream::StreamExt;
use lapin::options::{
	BasicAckOptions, BasicConsumeOptions, BasicNackOptions, BasicQosOptions, BasicRejectOptions,
};
use lapin::types::FieldTable;
use lapin::Channel;

/// Consumes from a single queue with a bounded prefetch, so a crash never
/// leaves more than `prefetch` messages in flight to be redelivered.
pub struct RabbitMqConsumer {
	channel: Channel,
	consumer_tag: String,
	prefetch: u16,
}

impl RabbitMqConsumer {
	/// Build a consumer bound to `channel`. Does not start consuming until
	/// [`MessageConsumer::run`] is called.
	#[must_use]
	pub const fn new(channel: Channel, consumer_tag: String, prefetch: u16) -> Self {
		Self {
			channel,
			consumer_tag,
			prefetch,
		}
	}
}

#[async_trait]
impl MessageConsumer for RabbitMqConsumer {
	async fn run(&self, queue: &str, handler: ConsumeHandler) -> Result<(), BrokerError> {
		self.channel
			.basic_qos(self.prefetch, BasicQosOptions::default())
			.await
			.map_err(|e| BrokerError::Consumer(e.to_string()))?;

		let mut consumer = self
			.channel
			.basic_consume(
				queue.into(),
				self.consumer_tag.clone().into(),
				BasicConsumeOptions::default(),
				FieldTable::default(),
			)
			.await
			.map_err(|e| BrokerError::Consumer(e.to_string()))?;

		while let Some(delivery) = consumer.next().await {
			let delivery = delivery.map_err(|e| BrokerError::Consumer(e.to_string()))?;

			let delivery_id = delivery
				.properties
				.message_id()
				.as_ref()
				.map(ToString::to_string);

			let message = BrokerMessage {
				routing_key: delivery.routing_key.to_string(),
				payload: delivery.data.clone(),
				delivery_id,
			};

			let outcome = handler(message).await;

			let ack_result = match outcome {
				ConsumeOutcome::Ack => delivery.ack(BasicAckOptions::default()).await,
				ConsumeOutcome::Retry => delivery.nack(BasicNackOptions::default()).await,
				ConsumeOutcome::Reject => delivery.reject(BasicRejectOptions::default()).await,
			};

			ack_result.map_err(|e| BrokerError::Consumer(e.to_string()))?;
		}

		Ok(())
	}
}
