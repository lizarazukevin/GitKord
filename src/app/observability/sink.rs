//! Backend-agnostic log shipping interface.

use async_trait::async_trait;

/// A backend-agnostic log sink that can be gracefully shut down.
///
/// Implementations own the background task that ships logs to a remote
/// backend (e.g. Loki). `shutdown` signals the task to stop and awaits it
/// so any buffered log lines are flushed before the process exits.
///
/// Unlike the metrics traits (which are shared behind an [`Arc`] across many
/// components), a log sink has a single owner and a single shutdown call, so
/// it is returned by value and consumed by `shutdown(self)`.
#[async_trait]
pub trait LogSink: Send + Sync {
	/// Gracefully stop the log sink, flushing any buffered log lines.
	///
	/// Call this LAST during shutdown, after every other subsystem has
	/// stopped producing logs. Events emitted while it drains, or after it
	/// returns are best-effort and may be silently dropped.
	async fn shutdown(self);
}
