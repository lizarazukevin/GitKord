//! Domain model and traits for a `Discord` user linking their `GitHub` login.

use crate::error::AppError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct UserLink {
	pub discord_id: u64,
	pub github_login: String,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
	pub created_by: Option<String>,
	pub updated_by: Option<String>,
}

/// Persistence for the `Discord` ID to `GitHub` login mapping.
#[async_trait]
pub trait UserStore: Send + Sync {
	/// Insert or update the `GitHub` login linked to a `Discord` user.
	async fn upsert(&self, link: UserLink) -> Result<(), AppError>;
	/// Fetch the link for a `Discord` user, if one exists.
	async fn fetch_by_discord_id(&self, discord_id: u64) -> Result<Option<UserLink>, AppError>;
	/// Map each member of `github_logins` to its linked `Discord` user ID.
	async fn fetch_by_github_logins(
		&self,
		github_logins: &[String],
	) -> Result<HashMap<String, u64>, AppError>;
	/// Remove the link for a Discord user, if any.
	async fn delete(&self, discord_id: u64) -> Result<(), AppError>;
}
