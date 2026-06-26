//! Shared dependencies for slash command handlers.
//!
//! Grouping to keep dispatch function signature clean,
//! and new dependencies added in one location.

use std::sync::Arc;

use crate::db::{PrChannelMessageStore, SubscriptionStore, UserLinkStore};
use crate::github::client::GitHubClient;

pub struct AppState {
    /// PR message store for thread ID lookups and audit posting.
    pub pr_store: Arc<dyn PrChannelMessageStore>,

    /// Subscription store for validating repo subscriptions.
    pub sub_store: Arc<dyn SubscriptionStore>,

    /// User link store for Discord to GitHub username resolution.
    pub user_store: Arc<dyn UserLinkStore>,

    /// Authenticated GitHub API client .
    pub github: Arc<GitHubClient>,

    /// When `true`, the bot is running in local development mode.
    pub local_dev: bool,

    /// Public domain reachable via ngrok (only used when `local_dev` is true).
    pub public_domain: String,

    /// HMAC secret for verifying GitHub payloads (used to register webhooks in local dev).
    pub webhook_secret: String,
}
