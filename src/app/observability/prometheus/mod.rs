//! `Prometheus` integration for metric collection and export.

pub mod error;
pub mod exporter;
pub mod recorder;

pub use error::MetricsError;
pub use exporter::PrometheusExporter;
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder};
pub use recorder::PrometheusRecorder;

const EXPONENTIAL_SECONDS: &[f64] = &[
	0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Initialize the Prometheus metrics pipeline.
///
/// Configures exponential histogram buckets for any metric whose name matches
/// the suffixes (`_sec`, etc.) for duration, recorder installs into the global registry.
/// Bucket names follow the convention: <name>_<metric>_<unit>
///
/// # Arguments
/// * `environment` - The deployment environment (e.g. `"prod"`, `"local"`)
///
/// # Returns
/// A tuple of:
/// * `PrometheusRecorder` - implements [`MetricsRecorder`] for writing metrics.
/// * `PrometheusExporter` - implements [`MetricsRenderer`] for serving the `/metrics` endpoint.
///
/// # Errors
/// Returns [`MetricsError::Buckets`] if the bucket configuration is invalid,
/// or [`MetricsError::Install`] if a recorder is already installed globally.
pub fn init(environment: &str) -> Result<(PrometheusRecorder, PrometheusExporter), MetricsError> {
	let handle = PrometheusBuilder::new()
		.set_buckets_for_metric(Matcher::Suffix("_sec".to_owned()), EXPONENTIAL_SECONDS)
		.map_err(MetricsError::Buckets)?
		.install_recorder()
		.map_err(MetricsError::Install)?;

	Ok((
		PrometheusRecorder::new(environment.to_owned()),
		PrometheusExporter::new(handle),
	))
}
