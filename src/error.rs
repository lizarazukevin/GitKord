#![expect(dead_code)]

//! Unified error types for `DiGiBot`.
//!
//! [`AppError`] is returned by all Axum handlers.
//! [`IntoResponse`] impl Axum converts it directly into an HTTP response.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

/// Top-level error type for `DiGiBot`.
///
/// Each variant maps to specific HTTP status code via [`IntoResponse`],
/// keeping error-to-response logic in one place rather than scattered across handlers.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// Returned when a webhook payload's HMAC-SHA256 signature does not match.
    /// Maps to `401 Unauthorized` || rejected before processing.
    #[error("webhook signature verification failed")]
    InvalidSignature,

    /// Wraps errors from the GitHub REST API (via `octocrab`).
    /// Maps to `502 Bad Gateway` || request is fine, GitHub is the problem.
    #[error("GitHub API error: {0}")]
    GitHub(#[from] octocrab::Error),

    /// Wraps errors from the Serenity Discord client.
    /// Maps to `500 Internal Server Error`.
    #[error("Discord error: {0}")]
    Discord(#[from] Box<serenity::Error>),

    /// Catch-all for internal errors that don't fit variants.
    /// Maps to `500 Internal Server Error`.
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::InvalidSignature => StatusCode::UNAUTHORIZED,
            Self::GitHub(_) => StatusCode::BAD_GATEWAY,
            Self::Discord(_) | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, self.to_string()).into_response()
    }
}

/// Aliases
pub type Result<T> = std::result::Result<T, AppError>;
