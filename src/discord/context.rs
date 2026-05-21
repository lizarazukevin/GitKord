//! Shared dependencies for slash command handlers.
//!
//! Grouping to keep dispatch function signature clean,
//! and new dependencies added in one location.

use std::sync::Arc;

use crate::state::{PrMessageStore, SubscriptionStore, UserLinkStore};
use octocrab::Octocrab;

pub struct AppState {
    /// PR message store for thread ID lookups and audit posting.
    pub pr_store: Arc<dyn PrMessageStore>,

    /// Subscription store for validating repo subscriptions.
    pub sub_store: Arc<dyn SubscriptionStore>,

    /// User link store for Discord to GitHub username resolution.
    pub user_store: Arc<dyn UserLinkStore>,

    /// Authenticated GitHub API client .
    pub github: Arc<Octocrab>,

    /// Public URL this bot is reachable at, used when registering webhooks.
    pub webhook_url: String,

    /// HMAC secret passed to GitHub during webhook registration.
    pub webhook_secret: String,
}
