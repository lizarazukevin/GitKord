//! Graceful shutdown signal handling.

use tokio::select;
use tracing::{error, info};

/// Resolves when the process receives a shutdown signal:
/// - Ctrl+C
/// - Unix, SIGTERM
///
/// If installing a signal handler fails, that signal path is disabled
/// (logged, not panicked) rather than crashing the process. Other
/// signals, or the OS's default handling, still applies.
pub async fn shutdown_signal() {
	#[cfg(unix)]
	let terminate = terminate_signal();
	#[cfg(not(unix))]
	let terminate = std::future::pending::<()>();

	select! {
		() = ctrl_c_signal() => info!("received Ctrl+C, shutting down"),
		() = terminate => info!("received SIGTERM, shutting down"),
	}
}

async fn ctrl_c_signal() {
	if let Err(e) = tokio::signal::ctrl_c().await {
		error!(error = %e, "failed to listen for Ctrl+C, this shutdown path is disabled");
		std::future::pending::<()>().await;
	}
}

#[cfg(unix)]
async fn terminate_signal() {
	match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
		Ok(mut sig) => {
			sig.recv().await;
		}
		Err(e) => {
			error!(error = %e, "failed to install SIGTERM handler, this shutdown path is disabled");
			std::future::pending::<()>().await;
		}
	}
}
