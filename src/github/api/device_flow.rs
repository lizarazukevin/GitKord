#![allow(dead_code)]

//! `GitHub` App device-flow identity verification.
//!
//! Three endpoints, no OAuth scopes, no client secret:
//! - `POST https://github.com/login/device/code`
//! - `POST https://github.com/login/oauth/access_token`
//! - `GET  https://api.github.com/user`
//!
//! The bearer `device_code` and the resulting `ghu_` token are held as
//! [`SecretString`] and never logged or stored.

use crate::APP_NAME;
use reqwest::Client;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use std::fmt;

const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const USER_URL: &str = "https://api.github.com/user";

/// Payload result for `POST https://github.com/login/device/code`.
pub struct DeviceAuthorization {
	pub device_code: SecretString,
	pub user_code: String,
	pub verification_uri: String,
	/// GitHub's maximum lifetime. The service caps the poll deadline at ~14m30s.
	pub expires_in: u64,
	/// Initial poll interval; `slow_down` adds 5s per occurrence.
	pub interval: u64,
}

impl fmt::Debug for DeviceAuthorization {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("DeviceAuthorization")
			.field("device_code", &"[redacted]")
			.field("user_code", &self.user_code)
			.field("verification_uri", &self.verification_uri)
			.field("expires_in", &self.expires_in)
			.field("interval", &self.interval)
			.finish()
	}
}

/// Poll attempt against `POST /login/oauth/access_token`.
pub enum PollStep {
	Pending,
	SlowDown,
	Token(SecretString),
	Expired,
	Denied,
}

impl fmt::Debug for PollStep {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Pending => f.write_str("Pending"),
			Self::SlowDown => f.write_str("SlowDown"),
			Self::Token(_) => f.write_str("Token([redacted])"),
			Self::Expired => f.write_str("Expired"),
			Self::Denied => f.write_str("Denied"),
		}
	}
}

/// Identity returned by `GET /user`.
#[derive(Debug, Clone)]
pub struct GitHubIdentity {
	pub id: i64,
	pub login: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DeviceFlowError {
	#[error("device flow is disabled for this GitHub App")]
	Disabled,
	#[error("github device flow request failed: {0}")]
	Request(#[from] reqwest::Error),
	#[error("github device flow returned an unexpected error: {0}")]
	Unexpected(String),
}

#[derive(Clone)]
pub struct DeviceFlowClient {
	http: Client,
	client_id: String,
}

impl DeviceFlowClient {
	#[must_use]
	pub const fn new(http: Client, client_id: String) -> Self {
		Self { http, client_id }
	}

	pub async fn start(&self) -> Result<DeviceAuthorization, DeviceFlowError> {
		let response = self
			.http
			.post(DEVICE_CODE_URL)
			.header(reqwest::header::ACCEPT, "application/json")
			.form(&[("client_id", self.client_id.as_str())])
			.send()
			.await?;

		let status = response.status();
		let body = response.text().await?;

		if !status.is_success() {
			return Err(map_error_body(&body));
		}

		let parsed: DeviceCodeResponse =
			serde_json::from_str(&body).map_err(|e| DeviceFlowError::Unexpected(e.to_string()))?;

		Ok(DeviceAuthorization {
			device_code: SecretString::from(parsed.device_code),
			user_code: parsed.user_code,
			verification_uri: parsed.verification_uri,
			expires_in: parsed.expires_in,
			interval: parsed.interval,
		})
	}

	/// One poll attempt. Caller owns the interval, deadline and loop.
	pub async fn poll(&self, device_code: &SecretString) -> Result<PollStep, DeviceFlowError> {
		let response = self
			.http
			.post(ACCESS_TOKEN_URL)
			.header(reqwest::header::ACCEPT, "application/json")
			.form(&[
				("client_id", self.client_id.as_str()),
				("device_code", device_code.expose_secret()),
				("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
			])
			.send()
			.await?;

		let status = response.status();
		let body = response.text().await?;

		if !status.is_success() {
			return Err(map_error_body(&body));
		}

		let parsed: PollResponse =
			serde_json::from_str(&body).map_err(|e| DeviceFlowError::Unexpected(e.to_string()))?;

		if let Some(token) = parsed.access_token {
			return Ok(PollStep::Token(SecretString::from(token)));
		}

		match parsed.error.as_deref() {
			Some("authorization_pending") => Ok(PollStep::Pending),
			Some("slow_down") => Ok(PollStep::SlowDown),
			Some("expired_token") => Ok(PollStep::Expired),
			Some("access_denied") => Ok(PollStep::Denied),
			Some("device_flow_disabled") => Err(DeviceFlowError::Disabled),
			Some(other) => Err(DeviceFlowError::Unexpected(other.to_owned())),
			None => Err(DeviceFlowError::Unexpected(
				"empty access_token response".to_owned(),
			)),
		}
	}

	/// Fetch `id` and `login` for a just-issued token.
	pub async fn fetch_identity(
		&self,
		token: &SecretString,
	) -> Result<GitHubIdentity, DeviceFlowError> {
		let response = self
			.http
			.get(USER_URL)
			.header(reqwest::header::USER_AGENT, APP_NAME)
			.header(reqwest::header::ACCEPT, "application/vnd.github+json")
			.header("X-GitHub-Api-Version", "2022-11-28")
			.bearer_auth(token.expose_secret())
			.send()
			.await?;

		let status = response.status();
		let body = response.text().await?;

		if !status.is_success() {
			return Err(map_error_body(&body));
		}

		let parsed: UserResponse =
			serde_json::from_str(&body).map_err(|e| DeviceFlowError::Unexpected(e.to_string()))?;

		Ok(GitHubIdentity {
			id: parsed.id,
			login: parsed.login,
		})
	}
}

/// GitHub returns JSON errors on both `github.com` and `api.github.com`; treat
/// a `device_flow_disabled` error body as the dedicated variant.
fn map_error_body(body: &str) -> DeviceFlowError {
	#[derive(Deserialize)]
	struct ErrorBody {
		error: Option<String>,
		error_description: Option<String>,
	}

	let parsed: Option<ErrorBody> = serde_json::from_str(body).ok();

	if let Some(ErrorBody {
		error: Some(error),
		error_description,
	}) = parsed
	{
		if error == "device_flow_disabled" {
			return DeviceFlowError::Disabled;
		}
		return DeviceFlowError::Unexpected(error_description.unwrap_or(error));
	}

	DeviceFlowError::Unexpected(body.to_owned())
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
	device_code: String,
	user_code: String,
	verification_uri: String,
	expires_in: u64,
	interval: u64,
}

#[derive(Deserialize)]
struct PollResponse {
	access_token: Option<String>,
	error: Option<String>,
}

#[derive(Deserialize)]
struct UserResponse {
	id: i64,
	login: String,
}
