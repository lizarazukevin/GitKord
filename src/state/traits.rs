//! Store trait abstractions for `DiGiBot`.
//!
//! High-level handlers depend on these traits, not on concrete database types.
//! This means the backing store can be swapped (`SQLite` → `PostgreSQL`, or an
//! in-memory store for tests) without touching any handler code.

use async_trait::async_trait;

use crate::error::AppError;

// ── PR message store ──────────────────────────────────────────────────────────

/// Associates a GitHub PR with the Discord message that represents it.
#[derive(Debug, Clone)]
pub struct PrMessage {
    /// GitHub repository in `owner/name` format.
    pub repo: String,

    /// Pull request number.
    pub pr_number: u64,

    /// Discord channel the message was posted in.
    pub channel_id: u64,

    /// Discord message ID — used to edit the message in place on future events.
    pub message_id: u64,
}

/// Persist and retrieve the Discord message ID for each open PR.
#[async_trait]
pub trait PrMessageStore: Send + Sync {
    /// Save the Discord message ID for a PR.
    ///
    /// If a record already exists for `(repo, pr_number)` it is replaced.
    async fn upsert(&self, record: PrMessage) -> Result<(), AppError>;

    /// Look up the Discord message for a PR.
    ///
    /// Returns `None` if no message has been posted for this PR yet.
    async fn get(&self, repo: &str, pr_number: u64) -> Result<Option<PrMessage>, AppError>;

    /// Remove the record for a PR once it is merged or deleted.
    async fn delete(&self, repo: &str, pr_number: u64) -> Result<(), AppError>;
}

// ── User link store ───────────────────────────────────────────────────────────

/// A link between a Discord user and their GitHub username.
#[derive(Debug, Clone)]
pub struct UserLink {
    /// Discord user snowflake ID.
    pub discord_id: u64,

    /// GitHub login handle.
    pub github_login: String,
}

/// Persist and retrieve Discord ↔ GitHub username mappings.
#[async_trait]
pub trait UserLinkStore: Send + Sync {
    /// Save or update a Discord ↔ GitHub link.
    async fn upsert(&self, link: UserLink) -> Result<(), AppError>;

    /// Look up a user's GitHub login by their Discord ID.
    async fn get_by_discord(&self, discord_id: u64) -> Result<Option<UserLink>, AppError>;

    /// Look up a Discord ID by GitHub login.
    async fn get_by_github(&self, github_login: &str) -> Result<Option<UserLink>, AppError>;

    /// Remove a link.
    async fn delete(&self, discord_id: u64) -> Result<(), AppError>;
}
