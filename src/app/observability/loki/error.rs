//! Unified errors for `Loki`.

/// Errors that can occur when setting up the Loki log exporter.
#[derive(Debug, thiserror::Error)]
pub enum LokiError {
	/// The endpoint URL could not be parsed.
	#[error("invalid Loki endpoint URL: {0}")]
	InvalidUrl(#[source] url::ParseError),

	/// Failed to build the Loki tracing layer.
	#[error("could not build Loki layer: {0}")]
	BuildError(#[source] tracing_loki::Error),
}
