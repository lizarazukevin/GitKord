//! Store trait abstractions.
//!
//! Handlers depend on these traits rather than concrete database types.

use crate::db::models::{PrChannelMessage, Subscription, UserLink};
use crate::error::Result;
use async_trait::async_trait;

#[async_trait]
pub trait PrChannelMessageStore: Send + Sync {
    /// Insert or replace the Discord message record for a PR.
    async fn upsert(&self, record: PrChannelMessage) -> Result<()>;

    /// Look up the Discord message for a PR. Returns `None` if not found.
    async fn get(&self, repo: &str, pr_number: u64) -> Result<Option<PrChannelMessage>>;

    /// Returns all mirrored PR messages for a repository PR pair.
    ///
    /// A single GitHub PR may fan out into multiple Discord channels
    /// across different guild subscriptions.
    async fn get_all_by_repo_and_pr(
        &self,
        repo: &str,
        pr_number: u64,
    ) -> Result<Vec<PrChannelMessage>>;

    /// Deletes a single PR record from within a channel.
    #[allow(dead_code)]
    async fn delete(&self, repo: &str, pr_number: u64, channel_id: u64) -> Result<()>;

    /// Removes all stored Discord message mappings for a PR.
    ///
    /// Used when a pull request is closed or cleaned up.
    #[expect(dead_code)]
    async fn delete_all_for_pr(&self, repo: &str, pr_number: u64) -> Result<()>;

    /// Lookup a PR record by its audit thread ID.
    /// Primarily used to infer context for assign/unassign when run inside a thread.
    async fn get_by_thread_id(&self, thread_id: u64) -> Result<Option<PrChannelMessage>>;
}

#[async_trait]
pub trait SubscriptionStore: Send + Sync {
    /// Insert or replace a subscription. One per repo per guild.
    async fn upsert(&self, subscription: Subscription) -> Result<()>;

    /// Look up subscriptions for a specific repo in a guild.
    async fn get_by_guild(&self, repo: &str, guild_id: u64) -> Result<Option<Subscription>>;

    /// Find all guilds subscribed to a repo.
    /// Called on every webhook event to find which channels to post to.
    async fn get_all_for_repo(&self, repo: &str) -> Result<Vec<Subscription>>;

    /// Remove a subscription.
    async fn delete(&self, repo: &str, guild_id: u64, channel_id: u64) -> Result<()>;

    /// Get installation ID for a repo.
    async fn get_installation_id(&self, repo: &str) -> Result<Option<u64>>;
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
