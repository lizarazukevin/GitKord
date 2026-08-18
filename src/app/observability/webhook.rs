//! Webhook event processing instrumentation.

use crate::app::observability::recorder::MetricsRecorder;
use crate::AppError;
use std::future::Future;
use std::time::Instant;
use tracing::error;

/// Wraps a webhook event future, recording its duration and any error it returns.
pub async fn observe_webhook_event<F, R>(
	event_name: &str,
	future: F,
	recorder: &dyn MetricsRecorder,
) -> Result<R, AppError>
where
	F: Future<Output = Result<R, AppError>>,
{
	let start = Instant::now();
	let result = future.await;

	recorder.record_webhook_duration(event_name, start.elapsed());

	if let Err(ref e) = result {
		recorder.record_webhook_error(event_name, e.error_type());
		error!(%event_name, error = %e, error_type = e.error_type(), "webhook event failed");
	}

	result
}
