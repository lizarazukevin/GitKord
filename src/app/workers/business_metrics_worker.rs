//! Business metrics worker.
//!
//! A self-contained periodic task that refreshes cached business metrics from
//! the data stores on a fixed interval.

use crate::app::observability::{observe, EventKind, LogContext, MetricsRecorder};
use crate::app::shutdown::shutdown_signal;
use crate::service::business_metrics::BusinessMetricsService;
use std::sync::Arc;
use std::time::Duration;
use tokio::select;
use tracing::{error, info};

pub struct BusinessMetricsWorker {
	service: Arc<BusinessMetricsService>,
	recorder: Arc<dyn MetricsRecorder>,
	interval: Duration,
}

impl BusinessMetricsWorker {
	#[must_use]
	pub fn new(
		service: Arc<BusinessMetricsService>,
		recorder: Arc<dyn MetricsRecorder>,
		interval: Duration,
	) -> Self {
		Self {
			service,
			recorder,
			interval,
		}
	}

	/// Aggregate business metrics on a fixed interval until a shutdown signal.
	///
	/// Per-metric failures are skipped inside [`BusinessMetricsService::aggregate`]
	/// (last-known-good values are retained), so this loop never exits on a
	/// transient errors (e.g. database).
	pub async fn run(self) {
		let mut ticker = tokio::time::interval(self.interval);
		loop {
			select! {
				_ = ticker.tick() => {
					let result = observe(
						EventKind::Aggregation,
						"aggregate_business_metrics",
						&LogContext::default(),
						async { self.service.aggregate().await },
						self.recorder.as_ref(),
					).await;
					if let Err(e) = result {
						error!(error = %e, "business metrics aggregation failed");
					}
				}
				() = shutdown_signal() => {
					info!("shutdown signal received, stopping business metrics worker");
					break;
				}
			}
		}
	}
}
