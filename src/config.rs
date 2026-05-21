//! Runtime configuration for `GitKord`.
//!
//! [`Config::from_env`] is the only place where environment variables are read.
//! Everywhere else receives values through function arguments or shared state.

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Config {
    /// Discord bot token from the Developer Portal.
    pub discord_token: String,

    /// HMAC secret used to verify incoming GitHub webhook payloads.
    pub github_webhook_secret: String,

    /// GitHub PAT for API calls (reviewer assignment, webhook reigstration).
    pub github_token: String,

    /// Publicly reachable URL for this bot (e.g. Railway domain or ngrok in dev).
    pub webhook_url: String,

    /// `SQLite` connection string. (Defaults to `sqlite://gitkord.db`).
    pub database_url: String,

    /// TCP port the Axum HTTP server listens on. Defaults to `3000`
    /// Change this if port 3000 is taken (e.g. by Vite dev server).
    pub port: u16,
}

impl Config {
    /// Load config from the environment. Fails fast if any required variable
    /// is missing or a value cannot be parsed.
    pub fn from_env() -> Result<Self, anyhow::Error> {
        Ok(Self {
            discord_token: require("DISCORD_TOKEN")?,
            github_webhook_secret: require("GITHUB_WEBHOOK_SECRET")?,
            github_token: require("GITHUB_TOKEN")?,
            webhook_url: require("WEBHOOK_URL")?,
            database_url: require("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite://gitkord.db?mode=rwc".into()),
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "3000".into())
                .parse::<u16>()
                .context("PORT must be a valid port number (1-65535)")?,
        })
    }
}

fn require(key: &str) -> Result<String> {
    std::env::var(key).with_context(|| format!("Missing environment variable {key}"))
}
