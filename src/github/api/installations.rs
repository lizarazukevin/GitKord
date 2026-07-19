//! `GitHub` app installation-related API calls.

use crate::AppError;
use octocrab::Octocrab;

/// Fetches the installation ID for a repository that has our `GitHub`
/// app installed, used to create installation/repo-specific clients.
pub async fn fetch_repository_installation_id(
    client: &Octocrab,
    owner: &str,
    repo: &str,
) -> Result<u64, AppError> {
    Ok(client
        .apps()
        .get_repository_installation(owner, repo)
        .await?
        .id
        .0)
}
