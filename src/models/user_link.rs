//! Domain model and traits for a `Discord` user linking their `GitHub` account.
use crate::error::AppError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct UserLink {
	pub discord_id: u64,
	/// GitHub's current casing. `None` only for a row whose login was removed (unlinked).
	pub github_login: Option<String>,
	/// Numeric GitHub user ID. `None` for legacy, unverified rows.
	#[allow(dead_code)]
	pub github_user_id: Option<i64>,
	/// Set only when ownership was proven via device flow.
	#[allow(dead_code)]
	pub github_verified_at: Option<DateTime<Utc>>,
	#[allow(dead_code)]
	pub created_at: DateTime<Utc>,
	#[allow(dead_code)]
	pub updated_at: DateTime<Utc>,
	#[allow(dead_code)]
	pub created_by: Option<String>,
	#[allow(dead_code)]
	pub updated_by: Option<String>,
}

/// Persistence for the `Discord` ID to `GitHub` identity mapping.
#[async_trait]
pub trait UserStore: Send + Sync {
	/// Legacy, unauthenticated upsert by login. Kept until commit 4 removes
	/// `/link <username>`.
	async fn upsert(&self, link: UserLink) -> Result<(), AppError>;

	/// Authenticated upsert. Clears any other row's login matching
	/// `github_login` case-insensitively, then writes the verified identity in
	/// one transaction.
	///
	/// Returns [`AppError::UserLinkConflict`] when the GitHub ID
	/// already belongs to another Discord user.
	#[allow(dead_code)]
	async fn upsert_verified(
		&self,
		discord_id: u64,
		github_user_id: i64,
		github_login: &str,
	) -> Result<(), AppError>;

	/// Fetch the link for a `Discord` user, if one exists.
	async fn fetch_by_discord_id(&self, discord_id: u64) -> Result<Option<UserLink>, AppError>;

	/// Map each member of `github_logins` to its linked `Discord` user ID.
	async fn fetch_by_github_logins(
		&self,
		github_logins: &[String],
	) -> Result<HashMap<String, u64>, AppError>;

	/// Null the GitHub columns but keep the row (forward-compatible with more
	/// providers). No-op when nothing is linked.
	#[allow(dead_code)]
	async fn unlink_github(&self, discord_id: u64) -> Result<(), AppError>;

	/// Delete the row entirely. Legacy `/unlink` only; removed in commit 4.
	async fn delete(&self, discord_id: u64) -> Result<(), AppError>;

	/// Count rows that currently have a GitHub identity.
	async fn count_registered(&self) -> Result<u64, AppError>;
}
