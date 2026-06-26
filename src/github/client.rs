//! Authenticated client to communicate with `GitHub`.

use crate::error::AppError;
use jsonwebtoken::EncodingKey;
use octocrab::Octocrab;

/// Wraps an `Octocrab` client and carries the authentication mode.
///
/// In production, the inner client is an app-level `Octocrab` and
/// [`GitHubClient::installation_client`] derives an installation-scoped client.
/// In local dev mode, the inner client is PAT-based and
/// [`GitHubClient::installation_client`] returns it directly (installation ID is ignored).
#[derive(Clone)]
pub struct GitHubClient {
    inner: Octocrab,
    local_dev: bool,
}

impl GitHubClient {
    /// Build an authenticated `GitHubClient`.
    ///
    /// In production, builds an app-level client using `app_id` and `private_key_pem`.
    /// In local dev mode, builds a PAT-based client used directly for all API calls.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::GitHub`] if the client cannot be initialized.
    pub fn build(
        app_id: u64,
        private_key_pem: &str,
        local_dev: bool,
        pat_token: &str,
    ) -> crate::error::Result<Self> {
        let inner = if local_dev {
            Octocrab::builder()
                .personal_token(pat_token.to_owned())
                .build()
                .map_err(AppError::GitHub)?
        } else {
            let key = EncodingKey::from_rsa_pem(private_key_pem.as_bytes())
                .map_err(|e| AppError::Internal(anyhow::anyhow!("invalid private key: {e}")))?;

            Octocrab::builder()
                .app(app_id.into(), key)
                .build()
                .map_err(AppError::GitHub)?
        };

        Ok(Self { inner, local_dev })
    }

    /// Returns an installation-scoped client.
    ///
    /// In local dev mode, returns the PAT-based client directly (installation ID is ignored
    /// since there's no GitHub App installation). In production, derives an installation-scoped
    /// client from the app-level client.
    pub fn installation_client(&self, installation_id: u64) -> crate::error::Result<Octocrab> {
        if self.local_dev {
            return Ok(self.inner.clone());
        }

        self.inner
            .installation(installation_id.into())
            .map_err(AppError::GitHub)
    }

    /// Returns a reference to the inner `Octocrab` client.
    ///
    /// Used for direct API calls that don't need installation scoping
    /// (e.g. webhook registration, user lookup).
    pub const fn inner(&self) -> &Octocrab {
        &self.inner
    }
}

/// Look up installation ID for a specific repo.
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
