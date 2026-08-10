//! Command execution instrumentation.

use crate::app::observability::recorder::MetricsRecorder;
use std::future::Future;
use std::time::Instant;
use tracing::error;

/// Wraps a command future, recording its duration and any error it returns.
pub async fn observe_command<F, E>(
	command_name: &str,
	future: F,
	recorder: &dyn MetricsRecorder,
) -> Result<(), E>
where
	F: Future<Output = Result<(), E>>,
	E: std::fmt::Display,
{
	let start = Instant::now();
	let result = future.await;

	recorder.record_command_duration(command_name, start.elapsed());

	if let Err(ref result) = result {
		recorder.record_command_error(command_name, "internal");
		error!(%command_name, error = %result, "command failed");
	}

	result
}
