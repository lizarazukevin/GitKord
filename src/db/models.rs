//! Domain data models for db-agnostic consumption.

/// One row per open PR, tracks where Discord message and audit thread live.
#[derive(Debug, Clone)]
pub struct PrChannelMessage {
    pub repo: String,
    pub pr_number: u64,
    pub channel_id: u64,
    pub message_id: u64,
    pub thread_id: u64,
}

/// One row per repo per guild, tracks which channel gets PR messages.
/// Stores the `installation_id` to make repo-scoped API calls.
#[derive(Debug, Clone)]
pub struct Subscription {
    pub repo: String,
    pub guild_id: u64,
    pub channel_id: u64,
    pub installation_id: u64,
}

/// Maps a Discord user ID to a GitHub login.
#[derive(Debug, Clone)]
pub struct UserLink {
    pub discord_id: u64,
    pub github_login: String,
}
