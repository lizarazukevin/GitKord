//! `Prometheus`-backed implementation of [`MetricsRecorder`].

use crate::app::observability::MetricsRecorder;
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
	fn record_command_duration(&self, command_name: &str, duration: Duration) {
		histogram!("cmd_duration_sec",
			"cmd_name" => command_name.to_owned(),
			"env" => self.environment.clone(),
		)
		.record(duration.as_secs_f64());
	}

	fn record_command_error(&self, command_name: &str, error_type: &str) {
		counter!("cmd_errors_total",
			"cmd_name" => command_name.to_owned(),
			"error_type" => error_type.to_owned(),
			"environment" => self.environment.clone(),
		)
		.increment(1);
	}

	fn record_webhook_duration(&self, event_name: &str, duration: Duration) {
		histogram!("webhook_duration_sec",
			"event_name" => event_name.to_owned(),
			"env" => self.environment.clone(),
		)
		.record(duration.as_secs_f64());
	}

	fn record_webhook_error(&self, event_name: &str, error_type: &str) {
		counter!("webhook_errors_total",
			"event_name" => event_name.to_owned(),
			"error_type" => error_type.to_owned(),
			"environment" => self.environment.clone(),
		)
		.increment(1);
	}
}
