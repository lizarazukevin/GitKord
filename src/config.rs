//! Runtime configuration for `GitKord`.

use anyhow::{Context, Result};
use std::fmt;

pub const APP_NAME: &str = "GitKord";
pub const GITHUB_APP_URL: &str = "<https://github.com/apps/gitkord>";

/// Deployment environment, used as a metric label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
	Prod,
	Local,
}

impl From<bool> for Environment {
	fn from(local_dev: bool) -> Self {
		if local_dev {
			Self::Local
		} else {
			Self::Prod
		}
	}
}

impl fmt::Display for Environment {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Prod => write!(f, "prod"),
			Self::Local => write!(f, "local"),
		}
	}
}

#[derive(Clone)]
pub struct WebhookRegistrationConfig {
	pub local_dev: bool,
	pub public_domain: String,
	pub github_webhook_secret: String,
}

#[derive(Debug, Clone)]
pub struct EnvConfig {
	/// `Discord` bot token taken from developer portal.
	/// <https://discord.com/developers/home>
	pub discord_token: String,

	/// HMAC webhook secret used to verify incoming `GitHub` webhook payloads.
	pub github_webhook_secret: String,

	/// `GitHub` App ID from the app settings page. Prod only.
	pub github_app_id: u64,

	/// `GitHub` App private key contents (PEM format). Prod only.
	pub github_app_private_key: String,

	/// Database connection string (e.g. `Postgres`).
	pub database_url: String,

	/// TCP port the Axum HTTP server listens on. Defaults to `3000`.
	pub port: u16,

	/// Similar to `port` in the internal network sense (e.g. metrics).
	pub internal_port: u16,

	/// Local development mode when `true`. Dictates whether to use PAT or App ID.
	pub local_dev: bool,

	/// `GitHub` Personal Access Token (PAT). Local dev only.
	/// Used for API calls and webhook registration instead of `GitHub` App auth.
	pub github_token: String,

	/// Public domain reachable via ngrok. Local dev only.
	pub public_domain: String,
}

impl EnvConfig {
	/// Load config from the environment. Fails fast if any required variable
	/// is missing or a value cannot be parsed.
	pub fn from_env() -> Result<Self> {
		let local_dev = local_dev_flag();

		let discord_token = require_env("DISCORD_TOKEN")?;
		let github_webhook_secret = require_env("GITHUB_WEBHOOK_SECRET")?;
		let database_url = require_env("DATABASE_URL")?;
		let port = parse_port()?;
		let internal_port = 9090;

		let (github_app_id, github_app_private_key) = github_app_credentials(local_dev)?;
		let (github_token, public_domain) = local_dev_credentials(local_dev)?;

		Ok(Self {
			discord_token,
			github_webhook_secret,
			github_app_id,
			github_app_private_key,
			database_url,
			port,
			internal_port,
			local_dev,
			github_token,
			public_domain,
		})
	}

	/// Narrower config used in `/subscribe` to register and verify a repository's webhook.
	pub fn webhook_registration_config(&self) -> WebhookRegistrationConfig {
		WebhookRegistrationConfig {
			local_dev: self.local_dev,
			public_domain: self.public_domain.clone(),
			github_webhook_secret: self.github_webhook_secret.clone(),
		}
	}
}

fn local_dev_flag() -> bool {
	std::env::var("LOCAL_DEV")
		.ok()
		.is_some_and(|v| v == "true" || v == "1")
}

fn parse_port() -> Result<u16> {
	std::env::var("PORT")
		.unwrap_or_else(|_| "3000".into())
		.parse::<u16>()
		.context("PORT must be a valid port number")
}

/// `GitHub` app credentials required in production.
fn github_app_credentials(local_dev: bool) -> Result<(u64, String)> {
	if local_dev {
		return Ok((0, String::new()));
	}

	let app_id = require_env("GITHUB_APP_ID")?
		.parse::<u64>()
		.context("GITHUB_APP_ID must be a number")?;
	let private_key = require_env("GITHUB_APP_PRIVATE_KEY")?;

	Ok((app_id, private_key))
}

/// PAT + ngrok domain required in local dev.
fn local_dev_credentials(local_dev: bool) -> Result<(String, String)> {
	if !local_dev {
		return Ok((String::new(), String::new()));
	}

	let token = require_env("GITHUB_TOKEN")?;
	let public_domain = require_env("PUBLIC_DOMAIN")?;

	Ok((token, public_domain))
}

fn require_env(key: &str) -> Result<String> {
	std::env::var(key).with_context(|| format!("Missing environment variable {key}"))
}
