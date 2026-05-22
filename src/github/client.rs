//! Authenticated client to communicate with `GitHub`.

use crate::error::AppError;
use octocrab::Octocrab;

/// Build an authenticated `Octocrab` client from a personal access token.
///
/// # Errors
///
/// Returns [`AppError::GitHub`] if the client cannot be initialised.
pub fn build(token: &str) -> crate::error::Result<Octocrab> {
    Octocrab::builder()
        .personal_token(token.to_owned())
        .build()
        .map_err(AppError::GitHub)
}
