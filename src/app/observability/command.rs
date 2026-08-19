//! Command execution instrumentation.

use crate::app::observability::recorder::MetricsRecorder;
use crate::AppError;
use std::future::Future;
use std::time::Instant;
use tracing::error;

/// Wraps a command future, recording its duration and any error it returns.
pub async fn observe_command<F, R>(
	command_name: &str,
	future: F,
	recorder: &dyn MetricsRecorder,
) -> Result<R, AppError>
where
	F: Future<Output = Result<R, AppError>>,
{
	let start = Instant::now();
	let result = future.await;

	recorder.record_command_duration(command_name, start.elapsed());

	if let Err(ref e) = result {
		recorder.record_command_error(command_name, e.error_type());
		error!(%command_name, error = %e, error_type = e.error_type(), "command failed");
	}

	result
}
