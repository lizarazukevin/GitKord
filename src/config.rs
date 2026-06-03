//! Runtime configuration for `GitKord`.
//!
//! [`Config::from_env`] is the only place where environment variables are read.
//! Everywhere else receives values through function arguments or shared db.

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Config {
    /// Discord bot token from the Developer Portal.
    pub discord_token: String,

    /// HMAC secret used to verify incoming GitHub webhook payloads.
    pub github_webhook_secret: String,

    /// GitHub App ID from the app settings page.
    pub github_app_id: u64,

    /// GitHub App private key contents (PEM format).
    pub github_app_private_key: String,

    /// `Postgres` connection string.
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
            github_app_id: require("GITHUB_APP_ID")?
                .parse::<u64>()
                .context("GITHUB_APP_ID must be a number")?,
            github_app_private_key: require("GITHUB_APP_PRIVATE_KEY")?,
            database_url: require("DATABASE_URL")?,
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
