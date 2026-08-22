//! `Prometheus`-backed implementation of [`MetricsRecorder`].

use crate::app::observability::context::EventKind;
use crate::app::observability::MetricsRecorder;
use metrics::{counter, histogram};
use std::time::Duration;

/// Writes operation metrics to the global Prometheus registry, tagged with the
/// environment and the operation's event kind.
pub struct PrometheusRecorder {
	environment: String,
}

impl PrometheusRecorder {
	pub const fn new(environment: String) -> Self {
		Self { environment }
	}
}

impl MetricsRecorder for PrometheusRecorder {
	fn record_duration(&self, kind: EventKind, event_name: &str, duration: Duration) {
		histogram!("operation_duration_sec",
			"event_kind" => kind.as_str(),
			"event_name" => event_name.to_owned(),
			"env" => self.environment.clone(),
		)
		.record(duration.as_secs_f64());
	}

	fn record_error(&self, kind: EventKind, event_name: &str, error_type: &str) {
		counter!("operation_errors_total",
			"event_kind" => kind.as_str(),
			"event_name" => event_name.to_owned(),
			"error_type" => error_type.to_owned(),
			"env" => self.environment.clone(),
		)
		.increment(1);
	}
}
