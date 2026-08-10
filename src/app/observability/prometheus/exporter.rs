//! `Prometheus`-backed implementation of [`MetricsRenderer`].

use crate::app::observability::renderer::MetricsRenderer;
use metrics_exporter_prometheus::PrometheusHandle;

/// Owns the `PrometheusHandle` and provides a way to render the current metrics snapshot.
#[derive(Clone)]
pub struct PrometheusExporter {
	handle: PrometheusHandle,
}

impl PrometheusExporter {
	pub const fn new(handle: PrometheusHandle) -> Self {
		Self { handle }
	}
}

impl MetricsRenderer for PrometheusExporter {
	fn render(&self) -> String {
		self.handle.render()
	}
}
