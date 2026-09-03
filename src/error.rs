//! Unified error types for `GitKord`.
//!
//! [`AppError`] is returned by all Axum handlers.
//! [`IntoResponse`] impl Axum converts it directly into an HTTP response,
//! this way handlers can use ? freely without manual error mapping.

use crate::app::observability::loki::LokiError;
use crate::app::observability::prometheus::MetricsError;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
	/// Webhook payload signature did not match. Rejected before processing.
	#[error("webhook signature verification failed")]
	InvalidSignature,

	/// GitHub REST API error. Likely a bad request or permissions issue.
	#[error("GitHub API error: {0}")]
	GitHub(#[from] octocrab::Error),

	/// Serenity error. Boxed because [`serenity::Error`] is large.
	#[error("Discord error: {0}")]
	Discord(#[from] Arc<serenity::Error>),

	/// `Postgres` query error. `#[from]` lets `?` convert `sqlx::Error` directly.
	#[error("internal error: {0}")]
	Database(#[from] sqlx::Error),

	/// Schema migration failure. Categorized as `database` for metrics.
	#[error("database migration error: {0}")]
	Migration(#[from] sqlx::migrate::MigrateError),

	/// Metrics setup failure.
	#[error("observability error: {0}")]
	Metrics(#[from] MetricsError),

	/// Loki setup failure.
	#[error("observability error: {0}")]
	Loki(#[from] LokiError),

	/// User-facing validation/business error surfaced verbatim (no prefix).
	///
	/// Used for messages meant to be shown directly to a Discord user, e.g.
	/// "GitHub user not found". Kept distinct from [`AppError::Internal`] so
	/// these never leak an "internal error:" prefix into user replies.
	#[error("{0}")]
	Message(String),

	/// Catch-all for internal errors that don't fit variants.
	#[error("internal error: {0}")]
	Internal(#[from] anyhow::Error),
}

impl From<serenity::Error> for AppError {
	fn from(e: serenity::Error) -> Self {
		Self::Discord(Arc::new(e))
	}
}

impl AppError {
	/// Build a user-facing [`AppError::Message`] from anything string-like.
	pub fn message(msg: impl Into<String>) -> Self {
		Self::Message(msg.into())
	}

	/// For metrics, categorizes the error with a type.
	pub const fn error_type(&self) -> &'static str {
		match self {
			Self::InvalidSignature => "invalid_signature",
			Self::GitHub(_) => "github",
			Self::Discord(_) => "discord",
			Self::Database(_) | Self::Migration(_) => "database",
			Self::Metrics(_) => "metrics",
			Self::Loki(_) => "loki",
			Self::Message(_) => "validation",
			Self::Internal(_) => "internal",
		}
	}
}

impl IntoResponse for AppError {
	fn into_response(self) -> Response {
		let status = match &self {
			Self::InvalidSignature => StatusCode::UNAUTHORIZED,
			Self::GitHub(_) => StatusCode::BAD_GATEWAY,
			Self::Message(_) => StatusCode::BAD_REQUEST,
			Self::Metrics(_) | Self::Loki(_) => StatusCode::INTERNAL_SERVER_ERROR,
			Self::Discord(_) | Self::Database(_) | Self::Migration(_) | Self::Internal(_) => {
				StatusCode::INTERNAL_SERVER_ERROR
			}
		};

		// Only user-facing errors echo their message, else generic message.
		// Full details go to logs via `Display`/`source`.
		let body = match &self {
			Self::Message(_) | Self::InvalidSignature => self.to_string(),
			Self::GitHub(_)
			| Self::Discord(_)
			| Self::Database(_)
			| Self::Migration(_)
			| Self::Internal(_)
			| Self::Metrics(_)
			| Self::Loki(_) => {
				tracing::error!(error = %self, "internal error");
				"internal server error".to_string()
			}
		};

		(status, body).into_response()
	}
}

/// Formats the user-facing error.
pub fn format_error(header: &str, hint: Option<&str>) -> String {
	hint.map_or_else(
		|| format!("⚠️ **{header}**"),
		|hint| format!("⚠️ **{header}**\n{hint}"),
	)
}
