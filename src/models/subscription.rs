//! Domain model and traits for `Discord` channel subscriptions to a repository.

use crate::error::AppError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct Subscription {
	pub owner: String,
	pub project: String,
	pub guild_id: u64,
	pub channel_id: u64,
	pub installation_id: u64,
	#[allow(dead_code)]
	pub created_at: DateTime<Utc>,
	#[allow(dead_code)]
	pub updated_at: DateTime<Utc>,
	#[allow(dead_code)]
	pub created_by: Option<String>,
	#[allow(dead_code)]
	pub updated_by: Option<String>,
}

/// Persistence for `Discord` channel subscriptions to a repository's PR updates.
#[async_trait]
pub trait SubscriptionStore: Send + Sync {
	/// Insert or update a repository subscription to a channel.
	async fn upsert(&self, subscription: Subscription) -> Result<(), AppError>;
	/// Look up the stored `GitHub` app installation ID of a subscribed repository.
	async fn fetch_installation_id_by_owner_project(
		&self,
		owner: &str,
		project: &str,
	) -> Result<Option<u64>, AppError>;
	/// List every channel subscribed to a repository.
	async fn fetch_all_by_owner_project(
		&self,
		owner: &str,
		project: &str,
	) -> Result<Vec<Subscription>, AppError>;
	/// Remove a single channel subscription for a repository.
	async fn delete(
		&self,
		owner: &str,
		project: &str,
		guild_id: u64,
		channel_id: u64,
	) -> Result<(), AppError>;
	/// Remove every subscription for repository (used on app uninstall).
	async fn delete_all_by_owner_project(&self, owner: &str, project: &str)
		-> Result<(), AppError>;
}
