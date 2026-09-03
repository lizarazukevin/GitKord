//! Service layer for `/link` and `/unlink` commands.
//!
//! Verifies `GitHub` usernames and persists/removes the `Discord`‑to‑`GitHub`
//! mapping in the user store.

use std::sync::Arc;
use tracing::info;

use crate::error::{format_error, AppError};
use crate::github;
use crate::github::api::client::GitHubClient;
use crate::models::user_link::{UserLink, UserStore};
use chrono::Utc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkAction {
	Link,
	Unlink,
}

pub struct UserLinkRequest {
	pub github_login: String,
	pub discord_id: u64,
	pub action: LinkAction,
}

pub struct UserLinkService {
	user_store: Arc<dyn UserStore>,
	github: Arc<GitHubClient>,
}

impl UserLinkService {
	pub fn new(user_store: Arc<dyn UserStore>, github: Arc<GitHubClient>) -> Self {
		Self { user_store, github }
	}

	/// For link: verifies the `GitHub` user exists, then upserts the link.
	/// For unlink: deletes the link (no‑op if it doesn't exist).
	pub async fn handle(&self, req: UserLinkRequest) -> Result<String, AppError> {
		match req.action {
			LinkAction::Link => {
				match github::api::users::verify(&self.github, &req.github_login).await? {
					Some(verified) => {
						self.user_store
							.upsert(UserLink {
								discord_id: req.discord_id,
								github_login: verified.clone(),
								created_at: Utc::now(),
								updated_at: Utc::now(),
								created_by: None,
								updated_by: None,
							})
							.await?;

						info!("user link saved");

						Ok(format!(
							"Linked your Discord account to **{verified}** on GitHub."
						))
					}
					None => Err(AppError::message(format_error(
						"GitHub user not found",
						Some("Check the username and try again."),
					))),
				}
			}
			LinkAction::Unlink => {
				self.user_store.delete(req.discord_id).await?;

				info!("user link removed");

				Ok("Your Discord to GitHub link has been removed.".into())
			}
		}
	}
}
