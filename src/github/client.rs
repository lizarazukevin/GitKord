//! Authenticated client to communicate with `GitHub`.

use crate::error::AppError;
use jsonwebtoken::EncodingKey;
use octocrab::Octocrab;

/// Build an authenticated `Octocrab` client for a GitHub app.
///
/// The returned client works at app-level, uses an `installation_client`
/// to get a repo-scoped client for actual API calls.
///
/// # Errors
///
/// Returns [`AppError::GitHub`] if the client cannot be initialized.
pub fn build(app_id: u64, private_key_pem: &str) -> crate::error::Result<Octocrab> {
    let key = EncodingKey::from_rsa_pem(private_key_pem.as_bytes())
        .map_err(|e| AppError::Internal(anyhow::anyhow!("invalid private key: {e}")))?;

    Octocrab::builder()
        .app(app_id.into(), key)
        .build()
        .map_err(AppError::GitHub)
}

/// Look up installation ID for specific repo.
///
/// Called once at subscribe time and stored so future API calls
/// skip this lookup call.
pub async fn get_installation_id(
    client: &Octocrab,
    owner: &str,
    repo: &str,
) -> crate::error::Result<u64> {
    let installation = client
        .apps()
        .get_repository_installation(owner, repo)
        .await
        .map_err(AppError::GitHub)?;

    Ok(installation.id.0)
}

/// Builds installation-scoped client from stored installation ID.
///
/// Cheaper than using `installation_client` as it skips API lookup.
pub fn installation_client_from_id(
    client: &Octocrab,
    installation_id: u64,
) -> crate::error::Result<Octocrab> {
    client
        .installation(installation_id.into())
        .map_err(AppError::GitHub)
}
