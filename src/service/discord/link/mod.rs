//! Service layer for `/link` and `/unlink` commands.
//!
//! `/link` runs GitHub's device flow in two phases the command handler drives:
//! `start_link` returns a [`StartedFlow`] after deferring nothing and touching
//! no interaction state, `complete_link` polls GitHub and writes the link.

mod device_flow;

use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::app::observability::MetricsRecorder;
use crate::error::AppError;
use crate::github::api::device_flow::DeviceFlowClient;
use crate::models::user_link::UserStore;

pub use device_flow::{DeviceCodeDisplay, LinkOutcome, StartLinkError, StartedFlow};
use device_flow::{DeviceFlowRunner, PollResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserLinkAction {
	Link,
	Unlink,
}

pub struct UserLinkRequest {
	pub discord_id: u64,
	pub action: UserLinkAction,
}

pub struct UserLinkService {
	user_store: Arc<dyn UserStore>,
	flow: DeviceFlowRunner,
	recorder: Arc<dyn MetricsRecorder>,
}

impl UserLinkService {
	pub fn new(
		user_store: Arc<dyn UserStore>,
		device_flow: DeviceFlowClient,
		recorder: Arc<dyn MetricsRecorder>,
		shutdown: CancellationToken,
	) -> Self {
		Self {
			user_store,
			flow: DeviceFlowRunner::new(device_flow, Arc::clone(&recorder), shutdown),
			recorder,
		}
	}

	/// Phase 1: request a device code and return the display
	/// payload. Acquires the concurrency permit for the whole flow.
	pub async fn start_link(&self) -> Result<StartedFlow, StartLinkError> {
		self.flow.start_link().await
	}

	/// Phase 2: poll until token, then persist the identity. On user-caused
	/// terminals (`Expired`, `Denied`, `Timeout`, `Shutdown`) the outcome
	/// is `Ok`; real failures are `Err`.
	pub async fn complete_link(
		&self,
		discord_id: u64,
		flow: StartedFlow,
	) -> Result<LinkOutcome, AppError> {
		match self.flow.poll(flow).await? {
			PollResult::Verified { id, login } => {
				match self
					.user_store
					.upsert_verified(discord_id, id, &login)
					.await
				{
					Ok(()) => {
						self.recorder.record_link_outcome("success", "linked");
						Ok(LinkOutcome::Success { login })
					}
					Err(AppError::UserLinkConflict) => {
						self.recorder.record_link_outcome("failed", "conflict");
						Err(AppError::UserLinkConflict)
					}
					Err(e) => {
						self.recorder
							.record_link_outcome("failed", "database_error");
						Err(e)
					}
				}
			}
			PollResult::Expired => Ok(LinkOutcome::Expired),
			PollResult::Denied => Ok(LinkOutcome::Denied),
			PollResult::Timeout => Ok(LinkOutcome::Timeout),
			PollResult::Shutdown => Ok(LinkOutcome::Shutdown),
		}
	}

	/// `/unlink`. Reports whether a link was actually cleared so the handler
	/// can pick a message.
	pub async fn unlink(&self, discord_id: u64) -> Result<(), AppError> {
		self.user_store.unlink_github(discord_id).await?;
		info!("unlink: discord_id={}", discord_id);
		Ok(())
	}
}
