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

    /// GitHub App ID from the app settings page. Not needed in local dev.
    pub github_app_id: u64,

    /// GitHub App private key contents (PEM format). Not needed in local dev.
    pub github_app_private_key: String,

    /// `Postgres` connection string.
    pub database_url: String,

    /// TCP port the Axum HTTP server listens on. Defaults to `3000`
    /// Change this if port 3000 is taken (e.g. by Vite dev server).
    pub port: u16,

    /// When `true`, the bot runs in local development mode:
    /// - Uses `GITHUB_TOKEN` (PAT) for API calls instead of GitHub App auth
    /// - Registers webhooks on repos during `/subscribe` via PAT
    /// - Requires `PUBLIC_DOMAIN` for the ngrok URL
    /// - Skips HMAC signature verification when the header is absent
    pub local_dev: bool,

    /// GitHub Personal Access Token — only required when `LOCAL_DEV=true`.
    /// Used for API calls and webhook registration instead of GitHub App auth.
    pub github_token: String,

    /// Public domain reachable via ngrok — only required when `LOCAL_DEV=true`.
    /// Used to register webhooks pointing at the local server.
    pub public_domain: String,
}

impl Config {
    /// Load config from the environment. Fails fast if any required variable
    /// is missing or a value cannot be parsed.
    pub fn from_env() -> Result<Self, anyhow::Error> {
        let local_dev = std::env::var("LOCAL_DEV")
            .ok()
            .is_some_and(|v| v == "true" || v == "1");

        Ok(Self {
            discord_token: require("DISCORD_TOKEN")?,
            github_webhook_secret: require("GITHUB_WEBHOOK_SECRET")?,
            github_app_id: if local_dev {
                0
            } else {
                require("GITHUB_APP_ID")?
                    .parse::<u64>()
                    .context("GITHUB_APP_ID must be a number")?
            },
            github_app_private_key: if local_dev {
                String::new()
            } else {
                require("GITHUB_APP_PRIVATE_KEY")?
            },
            database_url: require("DATABASE_URL")?,
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "3000".into())
                .parse::<u16>()
                .context("PORT must be a valid port number (1-65535)")?,
            local_dev,
            github_token: if local_dev {
                require("GITHUB_TOKEN")?
            } else {
                String::new()
            },
            public_domain: if local_dev {
                require("PUBLIC_DOMAIN")?
            } else {
                String::new()
            },
        })
    }
}

fn require(key: &str) -> Result<String> {
    std::env::var(key).with_context(|| format!("Missing environment variable {key}"))
}
