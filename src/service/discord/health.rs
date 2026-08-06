//! Service layer for the `/health` command.

use std::sync::Arc;
use tracing::error;

use crate::config::APP_NAME;
use crate::github::api::client::GitHubClient;

pub struct HealthService {
	github: Arc<GitHubClient>,
}

impl HealthService {
	pub const fn new(github: Arc<GitHubClient>) -> Self {
		Self { github }
	}

	/// Check both services and return a user‑facing status message.
	///
	/// The `Discord` bot is always "online" when this runs, so the only
	/// real check is whether the `GitHub` App responds.
	pub async fn handle(&self) -> String {
		let github_status = match self.github.authenticated().current().app().await {
			Ok(_) => "🟢",
			Err(e) => {
				error!(error = %e, "GitHub App health check failed");
				"🔴"
			}
		};

		format!(
			"**{APP_NAME}** is online and healthy!\n \
             `🟢 Bot`  **·**  `{github_status} App`"
		)
	}
}
