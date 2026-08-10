//! Backend-agnostic metrics snapshot rendering interface.

pub trait MetricsRenderer: Send + Sync {
	/// Return the metrics in a format suitable for the `/metrics` endpoint.
	fn render(&self) -> String;
}
