//! Backend-agnostic metric recording interface.

use std::time::Duration;

pub trait MetricsRecorder: Send + Sync {
	/// Record the duration of a command execution.
	fn record_command_duration(&self, command_name: &str, duration: Duration);
	/// Record an error that occurred during a command.
	fn record_command_error(&self, command_name: &str, error_type: &str);
}
