//! Backend-agnostic metric recording interface.

use crate::app::observability::EventKind;
use std::time::Duration;

pub trait MetricsRecorder: Send + Sync {
	fn record_duration(&self, kind: EventKind, name: &str, duration: Duration);
	fn record_error(&self, kind: EventKind, name: &str, error_type: &str);
}
