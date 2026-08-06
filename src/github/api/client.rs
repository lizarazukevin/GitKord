//! Client objects to communicate with `GitHub`.
use crate::error::AppError;
use anyhow::anyhow;
use jsonwebtoken::EncodingKey;
use octocrab::Octocrab;

#[derive(Clone)]
enum AuthMode {
	/// Production: app-level client, scoped per-installation on demand.
	App(Octocrab),
	/// Local Dev: one Personal-Access-Token-based client.
	Pat(Octocrab),
}

#[derive(Clone)]
pub struct GitHubClient {
	auth_mode: AuthMode,
	/// Unauthenticated client for public, rate-limited lookups.
	anonymous: Octocrab,
}

impl GitHubClient {
	/// Build an authenticated `GitHubClient`.
	///
	/// In production, builds an app-level client using `app_id` and `private_key_pem`.
	/// In local dev `auth_mode`, builds a PAT-based client used directly for all API calls.
	pub fn new(
		app_id: u64,
		private_key_pem: &str,
		pat_token: &str,
		local_dev: bool,
	) -> Result<Self, AppError> {
		let auth_mode = if local_dev {
			let client = Octocrab::builder()
				.personal_token(pat_token.to_owned())
				.build()?;
			AuthMode::Pat(client)
		} else {
			// `jsonwebtoken::Error` has no `AppError` conversion; surface it as an internal error.
			let key = EncodingKey::from_rsa_pem(private_key_pem.as_bytes())
				.map_err(|e| AppError::Internal(anyhow!("invalid GitHub App private key: {e}")))?;
			let client = Octocrab::builder().app(app_id.into(), key).build()?;
			AuthMode::App(client)
		};

		Ok(Self {
			auth_mode,
			anonymous: Octocrab::default(),
		})
	}

	/// Returns a reference to the authenticated/base `Octocrab` client.
	/// Use for direct API calls that don't need installation scoping,
	/// or for PAT-authenticated admin actions (e.g. register webhook).
	pub const fn authenticated(&self) -> &Octocrab {
		match &self.auth_mode {
			AuthMode::App(client) | AuthMode::Pat(client) => client,
		}
	}

	/// A client authorized to act as a specific `GitHub` App installation.
	/// Use this once you have an installation ID (stored as part of `/subscribe`).
	pub fn scoped_to_installation(&self, installation_id: u64) -> Result<Octocrab, AppError> {
		match &self.auth_mode {
			AuthMode::Pat(client) => Ok(client.clone()),
			AuthMode::App(client) => Ok(client.installation(installation_id.into())?),
		}
	}

	/// Unauthenticated client for public lookups, requires no credentials.
	pub const fn anonymous(&self) -> &Octocrab {
		&self.anonymous
	}
}
