//! Domain model and traits for a PR message posted to the subscribed `Discord` channel.

use crate::error::AppError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct PrMessage {
	/// Full repository name in `owner/project` form.
	pub repository: String,
	pub pr: u64,
	pub channel_id: u64,
	pub message_id: u64,
	pub thread_id: u64,
	#[allow(dead_code)]
	pub created_at: DateTime<Utc>,
	#[allow(dead_code)]
	pub updated_at: DateTime<Utc>,
	#[allow(dead_code)]
	pub created_by: Option<String>,
	#[allow(dead_code)]
	pub updated_by: Option<String>,
}

/// Persistence for a PR and its message sent on `Discord`.
#[async_trait]
pub trait PrStore: Send + Sync {
	/// Insert or update the message record for a stored PR.
	async fn upsert(&self, record: PrMessage) -> Result<(), AppError>;
	/// Look up the PR message whose audit thread has the given ID, if any.
	async fn fetch_by_thread_id(&self, thread_id: u64) -> Result<Option<PrMessage>, AppError>;
	/// List every stored message for a repository's PR.
	async fn fetch_all_by_repo_and_pr(
		&self,
		repo: &str,
		pr_number: u64,
	) -> Result<Vec<PrMessage>, AppError>;
}
