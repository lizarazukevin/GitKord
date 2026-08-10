//! Unified errors for `Prometheus`.

use metrics_exporter_prometheus::BuildError;

/// Errors that can occur when setting up the Prometheus metrics pipeline.
#[derive(Debug, thiserror::Error)]
pub enum MetricsError {
	/// Failed to configure histogram buckets.
	#[error("failed to set buckets: {0}")]
	Buckets(#[source] BuildError),

	/// Failed to install the global recorder.
	#[error("could not install recorder: {0}")]
	Install(#[source] BuildError),
}
