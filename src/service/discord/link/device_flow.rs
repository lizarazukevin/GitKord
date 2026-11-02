//! Device-flow state machine for `/link`.
//!
//! Two entry points: [`DeviceFlowRunner::start_link`] (request a code) and
//! [`DeviceFlowRunner::poll`] (wait for the token, fetch identity). Persist
//! happens in [`super::UserLinkService::complete_link`].

use std::sync::Arc;
use std::time::{Duration, Instant};

use secrecy::SecretString;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::app::observability::MetricsRecorder;
use crate::error::AppError;
use crate::github::api::device_flow::{
	DeviceAuthorization, DeviceFlowClient, DeviceFlowError, PollStep,
};

/// Cap on concurrent in-flight flows. Guards against abuse; a pending flow
/// costs ~1–2 KB.
const MAX_CONCURRENT_FLOWS: usize = 5;

/// Interaction tokens are valid for 15 minutes; cap the poll deadline at
/// 14m30s so the final edit still lands.
const DEADLINE_CEILING: Duration = Duration::from_secs(14 * 60 + 30);

/// `slow_down` adds this much to the current interval.
const SLOW_DOWN_ADD: Duration = Duration::from_secs(5);

/// Display-safe subset of [`DeviceAuthorization`]. `device_code` is a bearer
/// secret and deliberately not part of this type.
pub struct DeviceCodeDisplay {
	pub user_code: String,
	pub verification_uri: String,
	pub expires_in: u64,
}

/// A running device flow. Owns the concurrency permit; dropping it abandons
/// the flow.
pub struct StartedFlow {
	pub display: DeviceCodeDisplay,
	pub(super) auth: DeviceAuthorization,
	pub(super) permit: OwnedSemaphorePermit,
	pub(super) cancel: CancellationToken,
}

/// `start_link` outcome.
pub enum StartLinkError {
	/// Concurrency cap saturated. User can retry shortly.
	Busy,
	/// Process is shutting down. User can retry after restart.
	ShuttingDown,
	/// GitHub rejected the request. Handler converts to `AppError`.
	Github(DeviceFlowError),
}

impl From<tokio::sync::TryAcquireError> for StartLinkError {
	fn from(_err: tokio::sync::TryAcquireError) -> Self {
		Self::Busy
	}
}

pub enum LinkOutcome {
	Success { login: String },
	Expired,
	Denied,
	Timeout,
	Shutdown,
}

/// Internal: what `poll` returns before persist. Not exported.
pub(super) enum PollResult {
	Verified { id: i64, login: String },
	Expired,
	Denied,
	Timeout,
	Shutdown,
}

pub(super) struct DeviceFlowRunner {
	client: DeviceFlowClient,
	recorder: Arc<dyn MetricsRecorder>,
	shutdown: CancellationToken,
	cap: Arc<Semaphore>,
}

impl DeviceFlowRunner {
	pub(super) fn new(
		client: DeviceFlowClient,
		recorder: Arc<dyn MetricsRecorder>,
		shutdown: CancellationToken,
	) -> Self {
		Self {
			client,
			recorder,
			shutdown,
			cap: Arc::new(Semaphore::new(MAX_CONCURRENT_FLOWS)),
		}
	}

	pub(super) async fn start_link(&self) -> Result<StartedFlow, StartLinkError> {
		if self.shutdown.is_cancelled() {
			self.outcome("abandoned", "shutdown");
			return Err(StartLinkError::ShuttingDown);
		}

		// Acquire the concurrency permit; on failure report the flow as busy
		// and return an error the handler can surface to the Discord user.
		let Ok(permit) = self.cap.clone().try_acquire_owned() else {
			return Err(StartLinkError::Busy);
		};

		let auth = match self.client.start().await {
			Ok(a) => a,
			Err(DeviceFlowError::Disabled) => {
				error!("device flow is disabled on the GitHub App, enable it in App settings");
				self.outcome("failed", "device_flow_disabled");
				return Err(StartLinkError::Github(DeviceFlowError::Disabled));
			}
			Err(e) => {
				self.outcome("failed", "github_error");
				return Err(StartLinkError::Github(e));
			}
		};

		let display = DeviceCodeDisplay {
			user_code: auth.user_code.clone(),
			verification_uri: auth.verification_uri.clone(),
			expires_in: auth.expires_in,
		};
		Ok(StartedFlow {
			display,
			auth,
			permit,
			cancel: self.shutdown.clone(),
		})
	}

	/// Poll until token, deadline, terminal error, or shutdown. Does not
	/// persist.
	pub(super) async fn poll(&self, flow: StartedFlow) -> Result<PollResult, AppError> {
		let timeout = Duration::from_secs(flow.auth.expires_in).min(DEADLINE_CEILING);
		let now = Instant::now();
		let max_deadline = now.checked_add(DEADLINE_CEILING).unwrap_or(now);
		let deadline = now.checked_add(timeout).unwrap_or(max_deadline);
		let mut interval = Duration::from_secs(flow.auth.interval);

		loop {
			tokio::select! {
				biased;
				() = flow.cancel.cancelled() => {
					self.outcome("abandoned", "shutdown");
					return Ok(PollResult::Shutdown);
				}
				() = tokio::time::sleep(interval) => {}
			}

			if Instant::now() >= deadline {
				self.outcome("abandoned", "timeout");
				return Ok(PollResult::Timeout);
			}

			match self.client.poll(&flow.auth.device_code).await {
				Ok(PollStep::Pending) => {}
				Ok(PollStep::SlowDown) => {
					interval = interval.saturating_add(SLOW_DOWN_ADD);
				}
				Ok(PollStep::Expired) => {
					self.outcome("abandoned", "expired");
					return Ok(PollResult::Expired);
				}
				Ok(PollStep::Denied) => {
					self.outcome("abandoned", "denied");
					return Ok(PollResult::Denied);
				}
				Ok(PollStep::Token(token)) => return self.fetch(flow, &token).await,
				Err(e) => {
					self.outcome("failed", "github_error");
					return Err(AppError::Internal(anyhow::anyhow!(
						"github device flow poll: {e}"
					)));
				}
			}
		}
	}

	async fn fetch(&self, flow: StartedFlow, token: &SecretString) -> Result<PollResult, AppError> {
		// Permit is held until this function returns, flow is still
		// "in-flight" until we know the identity.
		let _permit = &flow.permit;
		match self.client.fetch_identity(token).await {
			Ok(ident) => Ok(PollResult::Verified {
				id: ident.id,
				login: ident.login,
			}),
			Err(e) => {
				self.outcome("failed", "github_error");
				Err(AppError::Internal(anyhow::anyhow!(
					"github device flow identity: {e}"
				)))
			}
		}
	}

	fn outcome(&self, outcome: &str, reason: &str) {
		self.recorder.record_link_outcome(outcome, reason);
	}
}
