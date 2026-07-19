//! Unified error types for `GitKord`.
//!
//! [`AppError`] is returned by all Axum handlers.
//! [`IntoResponse`] impl Axum converts it directly into an HTTP response,
//! this way handlers can use ? freely without manual error mapping.

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

impl AppError {
    /// Build a user-facing [`AppError::Message`] from anything string-like.
    pub fn message(msg: impl Into<String>) -> Self {
        Self::Message(msg.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::InvalidSignature => StatusCode::UNAUTHORIZED,
            Self::GitHub(_) => StatusCode::BAD_GATEWAY,
            Self::Message(_) => StatusCode::BAD_REQUEST,
            Self::Discord(_) | Self::Database(_) | Self::Internal(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };

        (status, self.to_string()).into_response()
    }
}

/// Formats the user-facing error.
pub fn format_error(header: &str, hint: Option<&str>) -> String {
    match hint {
        Some(hint) => format!("⚠️ **{}**\n{}", header, hint),
        None => format!("⚠️ **{}**", header),
    }
}
