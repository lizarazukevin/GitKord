//! GitHub delivery redelivery worker.
//!
//! A self-contained periodic worker that reconciles GitHub App hook deliveries
//! and re-posts failed delivery attempts.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::app::observability::{observe, EventKind, LogContext, MetricsRecorder};
use crate::app::shutdown::shutdown_signal;
use crate::AppError;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::{
	header::{self, HeaderMap, HeaderValue},
	Client,
};
use serde::{Deserialize, Serialize};
use tokio::select;
use tokio::time::MissedTickBehavior;
use tracing::{error, info, warn};

const API_BASE: &str = "https://api.github.com";
const API_VERSION: &str = "2026-03-10";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

/// Allow for clock drift and backdate `iat` between us and the issuer.
const JWT_IAT_SKEW_SECS: i64 = 60;
/// GitHub's maximum JWT lifetime is 10 minutes.
const JWT_TTL_SECS: i64 = 9 * 60;

/// Stop redelivering a GUID after this many failed attempts inside the window,
/// so a permanently broken delivery doesn't get hammered every tick.
const MAX_ATTEMPTS_PER_GUID: usize = 6;

#[derive(Serialize)]
struct Claims {
	iat: i64,
	exp: i64,
	iss: String,
}

/// One delivery attempt from `GET /app/hook/deliveries`. Redeliveries share the
/// original attempt's `guid`.
#[derive(Debug, Deserialize)]
struct Delivery {
	id: u64,
	guid: String,
	delivered_at: DateTime<Utc>,
	status: String,
}

/// Outcome counters for one reconcile pass, logged once per tick.
#[derive(Default)]
struct Summary {
	scanned: usize,
	skipped_ok: usize,
	gave_up: usize,
	redelivered: usize,
	failed_requests: usize,
}

pub struct WebhookRedeliveryWorker {
	recorder: Arc<dyn MetricsRecorder>,
	client: Client,
	key: EncodingKey,
	app_id: String,
	interval: Duration,
	lookback: Duration,
}

impl WebhookRedeliveryWorker {
	pub fn new(
		recorder: Arc<dyn MetricsRecorder>,
		app_id: String,
		private_key_pem: &str,
		interval: Duration,
		lookback: Duration,
	) -> Result<Self, AppError> {
		let key = EncodingKey::from_rsa_pem(private_key_pem.as_bytes())
			.context("not a valid RSA private key")?;

		let client = Client::builder()
			.user_agent("gitkord-reconciler")
			.default_headers(build_headers())
			.timeout(REQUEST_TIMEOUT)
			.build()
			.map_err(AppError::HttpClientBuild)?;

		Ok(Self {
			recorder,
			client,
			key,
			app_id,
			interval,
			lookback,
		})
	}

	/// Fetches deliveries inside the lookback window and redelivers each GUID
	/// that has no successful attempt. A failed redelivery request is counted
	/// and skipped so it can't block the rest.
	async fn reconcile(&self) -> Result<Summary, AppError> {
		let jwt = mint_jwt(&self.app_id, &self.key)?;

		let lookback = chrono::Duration::from_std(self.lookback)?;
		let cutoff = Utc::now()
			.checked_sub_signed(lookback)
			.context("lookback exceeds the representable time range")?;

		let deliveries = list_deliveries_since(&self.client, &jwt, cutoff).await?;

		let mut by_guid: HashMap<String, Vec<Delivery>> = HashMap::new();
		for d in deliveries {
			by_guid.entry(d.guid.clone()).or_default().push(d);
		}

		let mut summary = Summary {
			scanned: by_guid.len(),
			..Default::default()
		};

		for (guid, attempts) in by_guid {
			if attempts.iter().any(|a| a.status == "OK") {
				summary.skipped_ok = summary.skipped_ok.saturating_add(1);
				continue;
			}

			if attempts.len() >= MAX_ATTEMPTS_PER_GUID {
				warn!(%guid, attempts = attempts.len(), "attempt count exceeded, giving up");
				summary.gave_up = summary.gave_up.saturating_add(1);
				continue;
			}

			// Results are newest-first, retrying latest failed attempt
			let Some(latest) = attempts.first() else {
				continue;
			};
			let url = format!("{API_BASE}/app/hook/deliveries/{}/attempts", latest.id);
			match self
				.client
				.post(&url)
				.bearer_auth(&jwt)
				.send()
				.await
				.and_then(reqwest::Response::error_for_status)
			{
				Ok(_) => {
					info!(%guid, delivery_id = latest.id, "redelivery requested");
					summary.redelivered = summary.redelivered.saturating_add(1);
				}
				Err(e) => {
					warn!(%guid, error = %e, "redelivery request failed");
					summary.failed_requests = summary.failed_requests.saturating_add(1);
				}
			}
		}

		Ok(summary)
	}

	/// Reconcile on a fixed internal until shutdown signal.
	///
	/// Failures are logged and retried on next tick, loop never exits
	/// due to a transient error.
	pub async fn run(self) {
		let mut ticker = tokio::time::interval(self.interval);
		ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

		loop {
			select! {
				_ = ticker.tick() => {
					let result = observe(
						EventKind::WebhookRedelivery,
						"webhook_redelivery",
						&LogContext::default(),
						self.reconcile(),
						self.recorder.as_ref(),
					).await;
					match result {
						Ok(summary) => info!(
									redelivered = summary.redelivered,
									failed_requests = summary.failed_requests,
									skipped_ok = summary.skipped_ok,
									gave_up = summary.gave_up,
									scanned = summary.scanned,
									"webhook reconcile job finished"
								),
						Err(e) => error!(error = %e, "webhook redelivery job failed"),
					}
				}
				() = shutdown_signal() => {
					info!("shutdown signal received, stopping webhook redelivery worker");
					break;
				}
			}
		}
	}
}

fn mint_jwt(app_id: &str, key: &EncodingKey) -> Result<String> {
	let now = Utc::now().timestamp();
	let claims = Claims {
		iat: now.saturating_sub(JWT_IAT_SKEW_SECS),
		exp: now.saturating_add(JWT_TTL_SECS),
		iss: app_id.to_string(),
	};
	Ok(encode(&Header::new(Algorithm::RS256), &claims, key)?)
}

/// Pages through `GET /app/hook/deliveries` (newest first) until a delivery
/// older than `cutoff` is reached.
async fn list_deliveries_since(
	client: &Client,
	jwt: &str,
	cutoff: DateTime<Utc>,
) -> Result<Vec<Delivery>> {
	let mut next = Some(format!("{API_BASE}/app/hook/deliveries?per_page=100"));
	let mut out = Vec::new();

	'pages: while let Some(url) = next.take() {
		let resp = client
			.get(&url)
			.bearer_auth(jwt)
			.send()
			.await?
			.error_for_status()?;
		next = next_page_url(resp.headers());
		let page: Vec<Delivery> = resp.json().await?;

		for d in page {
			if d.delivered_at <= cutoff {
				break 'pages;
			}
			out.push(d);
		}
	}

	Ok(out)
}

/// Extracts the `rel="next"` URL from a `Link` header, if present.
fn next_page_url(headers: &HeaderMap) -> Option<String> {
	let link = headers.get(header::LINK)?.to_str().ok()?;
	link.split(',').find_map(|part| {
		if !part.contains("rel=\"next\"") {
			return None;
		}
		let start = part.find('<')?.saturating_add(1);
		let end = part.find('>')?;
		part.get(start..end).map(str::to_owned)
	})
}

fn build_headers() -> HeaderMap {
	let mut headers = HeaderMap::new();
	headers.insert(
		header::ACCEPT,
		HeaderValue::from_static("application/vnd.github+json"),
	);
	headers.insert(
		"X-GitHub-Api-Version",
		HeaderValue::from_static(API_VERSION),
	);
	headers
}
