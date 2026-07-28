//! `GitHub` user-related API calls.

use crate::github::api::client::GitHubClient;
use crate::AppError;

/// Verifies if a username exists, returns their login if `true`.
pub async fn verify(client: &GitHubClient, username: &str) -> Result<Option<String>, AppError> {
	let gh = client.anonymous();

	match gh.users(username).profile().await {
		Ok(user) => Ok(Some(user.login)),
		Err(octocrab::Error::GitHub { source, .. }) if source.status_code == 404 => Ok(None),
		Err(e) => Err(AppError::GitHub(e)),
	}
}
