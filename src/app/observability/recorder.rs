//! Backend-agnostic metric recording interface.

use crate::app::observability::context::EventKind;
use std::time::Duration;

pub trait MetricsRecorder: Send + Sync {
	/// Record the duration of an operation, tagged by its event kind.
	fn record_duration(&self, kind: EventKind, event_name: &str, duration: Duration);
	/// Record an error that occurred during an operation, tagged by its event kind.
	fn record_error(&self, kind: EventKind, event_name: &str, error_type: &str);
}
