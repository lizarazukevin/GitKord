//! Runtime configuration for `DiGiBot`.
//!
//! [`Config::from_env`] is the single source of truth for all env variables.
//! Nothing else in the codebase calls [`env::var`] directly, register here.

use anyhow::{Context, Result};

/// All runtime configuration, loaded once at startup.
///
/// Cloning is cheap, pass `Config` by clone into spawned tasks
/// rather than wrapping it in an `Arc`.
#[derive(Debug, Clone)]
pub struct Config {
    /// Discord bot token from the Developer Portal
    pub discord_token: String,

    /// Secret used to verify HMAC-SHA256 signatures on incoming webhook payloads
    pub github_webhook_secret: String,

    /// GitHub personal access token for REST API calls (reviewer assignment, etc.)
    pub github_token: String,

    /// `SQLite` database URL (e.g. `sqlite://digibot.db`)
    pub database_url: String,

    /// TCP port the Axum HTTP server listens on. Defaults to `3000`
    /// Port assignment conflicts with Vite API routing, change to another port if this happens
    pub port: u16,
}

impl Config {
    /// Load all configuration from the process environment.
    ///
    /// Returns the error immediately if any required variables are absent.
    /// A successful return guarantees all fields are valid for the process lifetime.
    pub fn from_env() -> Result<Self, anyhow::Error> {
        Ok(Self {
            discord_token: require("DISCORD_TOKEN")?,
            github_webhook_secret: require("GITHUB_WEBHOOK_SECRET")?,
            github_token: require("GITHUB_TOKEN")?,
            database_url: require("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite://digibot.db?mode=rwc".into()),
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
