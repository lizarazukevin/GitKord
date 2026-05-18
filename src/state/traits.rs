//! Store trait abstractions.
//!
//! Handlers depend on these traits rather than concrete database types.
//! Swapping `SQLite` for Postgres, or using in-memory store for tests,
//! only requires a new impl, no handler changes needed.

use async_trait::async_trait;

use crate::error::Result;

/// One row per open PR, tracks where Discord message and audit thread live.
#[derive(Debug, Clone)]
pub struct PrMessage {
    pub repo: String,
    pub pr_number: u64,
    pub channel_id: u64,
    pub message_id: u64,
    pub thread_id: u64,
}

#[async_trait]
pub trait PrMessageStore: Send + Sync {
    /// Insert or replace the Discord message record for a PR.
    async fn upsert(&self, record: PrMessage) -> Result<()>;

    /// Look up the Discord message for a PR. Returns `None` if not found.
    async fn get(&self, repo: &str, pr_number: u64) -> Result<Option<PrMessage>>;

    /// Delete the record when a PR is closed or merged.
    async fn delete(&self, repo: &str, pr_number: u64) -> Result<()>;

    /// Lookup a PR record by its audit thread ID.
    /// Primarily used to infer context for assign/unassign when run inside a thread.
    async fn get_by_thread_id(&self, thread_id: u64) -> Result<Option<PrMessage>>;
}

/// One row per repo per guild, tracks which channel gets PR messages.
#[derive(Debug, Clone)]
pub struct Subscription {
    pub repo: String,
    pub guild_id: u64,
    pub channel_id: u64,
}

#[async_trait]
pub trait SubscriptionStore: Send + Sync {
    /// Insert or replace a subscription. One per repo per guild.
    async fn upsert(&self, subscription: Subscription) -> Result<()>;

    /// Look up subscription for a specific repo and guild.
    async fn get(&self, repo: &str, guild_id: u64) -> Result<Option<Subscription>>;

    /// Find all guilds subscribed to a repo.
    /// Called on every webhook event to find which channels to post to.
    async fn get_all_for_repo(&self, repo: &str) -> Result<Vec<Subscription>>;

    /// Remove a subscription.
    async fn delete(&self, repo: &str, guild_id: u64) -> Result<()>;
}

/// Maps a Discord user ID to a GitHub login.
#[derive(Debug, Clone)]
pub struct UserLink {
    pub discord_id: u64,
    pub github_login: String,
}

#[async_trait]
pub trait UserLinkStore: Send + Sync {
    /// Insert of update a Discord to GitHub link.
    async fn upsert(&self, link: UserLink) -> Result<()>;

    /// Look up a GitHub login by Discord ID.
    async fn get_by_discord(&self, discord_id: u64) -> Result<Option<UserLink>>;

    /// Look up a Discord ID by GitHub login.
    #[allow(dead_code)]
    async fn get_by_github(&self, github_login: &str) -> Result<Option<UserLink>>;

    /// Remove a link.
    async fn delete(&self, discord_id: u64) -> Result<()>;
}
