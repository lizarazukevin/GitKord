//! Shared dependencies for GitHub's webhook handlers.

use crate::db::{PrChannelMessageStore, SubscriptionStore, UserLinkStore};
use crate::github::client::GitHubClient;
use serenity::all::Http;
use std::sync::Arc;

/// State shared across every webhook handler invocations.
/// Command-only dependencies live in `CommandContext`.
#[derive(Clone)]
pub struct WebhookState {
    /// HMAC secret for verifying GitHub payloads.
    pub secret: String,

    /// Serenity HTTP client for posting Discord messages.
    pub http: Arc<Http>,

    /// Authenticated GitHub API client for fetching PR details and reviews.
    pub github: Arc<GitHubClient>,

    /// Stores links between discord and GitHub.
    pub user_store: Arc<dyn UserLinkStore>,

    /// Stores PR message and thread IDs so events can edit in place.
    pub pr_store: Arc<dyn PrChannelMessageStore>,

    /// Stores channel subscription per repo and guild.
    pub sub_store: Arc<dyn SubscriptionStore>,
}
