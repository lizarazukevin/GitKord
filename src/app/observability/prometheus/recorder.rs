//! `Prometheus`-backed implementation of [`MetricsRecorder`].

use crate::app::observability::{EventKind, MetricsRecorder};
use metrics::{counter, histogram};
use std::time::Duration;

/// Writes command metrics to the global Prometheus registry, tagged with the environment.
pub struct PrometheusRecorder {
	environment: String,
}

impl PrometheusRecorder {
	pub const fn new(environment: String) -> Self {
		Self { environment }
	}
}

impl MetricsRecorder for PrometheusRecorder {
	fn record_duration(&self, kind: EventKind, name: &str, duration: Duration) {
		histogram!("operation_duration_sec",
		   "kind" => kind.as_str(),
		   "name" => name.to_owned(),
		   "env" => self.environment.clone(),
		)
		.record(duration.as_secs_f64());
	}

	fn record_error(&self, kind: EventKind, name: &str, error_type: &str) {
		counter!("operation_errors_total",
		   "kind" => kind.as_str(),
		   "name" => name.to_owned(),
		   "error_type" => error_type.to_owned(),
		   "env" => self.environment.clone(),
		)
		.increment(1);
	}
}
