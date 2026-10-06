//! Business metric aggregation and caching.
//!
//! The HTTP layer reads the cache with no database I/O and exposes the snapshot
//! to the public site, while the same counts are also published as Prometheus
//! gauges for internal observability.

use crate::models::newsletter_signup::NewsletterSignupStore;
use crate::models::pr_message::PrStore;
use crate::models::subscription::SubscriptionStore;
use crate::models::user_link::UserStore;
use crate::AppError;
use chrono::{DateTime, Utc};
use metrics::gauge;
use serde::Serialize;
use std::sync::Arc;
use tokio::join;
use tokio::sync::RwLock;
use tracing::warn;

/// A single aggregate count and when it was last successfully computed.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct MetricValue {
	pub value: u64,
	pub as_of: DateTime<Utc>,
}

/// The latest aggregate business metrics available to expose.
///
/// Each field is `Option<_>` so a metric stays `None` until its first successful
/// aggregation. Failed attempts don't overwrite previous cached value.
#[derive(Debug, Clone, Default, Serialize)]
pub struct BusinessMetrics {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub prs_served: Option<MetricValue>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub unique_users_registered: Option<MetricValue>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub discord_servers_installed: Option<MetricValue>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub repositories_subscribed: Option<MetricValue>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub community_signups: Option<MetricValue>,
}

/// Aggregates business metrics from the data stores into a cached snapshot.
pub struct BusinessMetricsService {
	pr_store: Arc<dyn PrStore>,
	user_store: Arc<dyn UserStore>,
	subscription_store: Arc<dyn SubscriptionStore>,
	newsletter_store: Arc<dyn NewsletterSignupStore>,
	environment: String,
	cache: RwLock<BusinessMetrics>,
}

impl BusinessMetricsService {
	#[must_use]
	pub fn new(
		pr_store: Arc<dyn PrStore>,
		user_store: Arc<dyn UserStore>,
		subscription_store: Arc<dyn SubscriptionStore>,
		newsletter_store: Arc<dyn NewsletterSignupStore>,
		environment: String,
	) -> Self {
		Self {
			pr_store,
			user_store,
			subscription_store,
			newsletter_store,
			environment,
			cache: RwLock::new(BusinessMetrics::default()),
		}
	}

	/// Recompute every metric into the cache.
	///
	/// Counts are gathered concurrently but resolved independently: a query that
	/// fails is logged and skipped, leaving that metric's last-known-good value
	/// in place (or `None` if it has never succeeded).
	pub async fn aggregate(&self) -> Result<(), AppError> {
		let (prs, users, guilds, repos, signups) = join!(
			self.pr_store.count_distinct_prs(),
			self.user_store.count_registered(),
			self.subscription_store.count_distinct_guilds(),
			self.subscription_store.count_distinct_repos(),
			self.newsletter_store.count_subscribed(),
		);

		let as_of = Utc::now();
		let mut cache = self.cache.write().await;

		match prs {
			Ok(value) => {
				publish_gauge("gitkord_prs_served", &self.environment, value);
				cache.prs_served = Some(MetricValue { value, as_of });
			}
			Err(e) => warn!(error = %e, "skipping aggregated metric `prs_served`"),
		}

		match users {
			Ok(value) => {
				publish_gauge("gitkord_unique_users_registered", &self.environment, value);
				cache.unique_users_registered = Some(MetricValue { value, as_of });
			}
			Err(e) => warn!(error = %e, "skipping aggregated metric `unique_users_registered`"),
		}

		match guilds {
			Ok(value) => {
				publish_gauge(
					"gitkord_discord_servers_installed",
					&self.environment,
					value,
				);
				cache.discord_servers_installed = Some(MetricValue { value, as_of });
			}
			Err(e) => warn!(error = %e, "skipping aggregated metric `discord_servers_installed`"),
		}

		match repos {
			Ok(value) => {
				publish_gauge("gitkord_repositories_subscribed", &self.environment, value);
				cache.repositories_subscribed = Some(MetricValue { value, as_of });
			}
			Err(e) => warn!(error = %e, "skipping aggregated metric `repositories_subscribed`"),
		}

		match signups {
			Ok(value) => {
				publish_gauge("gitkord_community_signups", &self.environment, value);
				cache.community_signups = Some(MetricValue { value, as_of });
			}
			Err(e) => warn!(error = %e, "skipping aggregated metric `community_signups`"),
		}

		drop(cache);
		Ok(())
	}

	/// Return a clone of the current cached snapshot. No database I/O.
	pub async fn snapshot(&self) -> BusinessMetrics {
		self.cache.read().await.clone()
	}
}

/// Publish a metric count as a Prometheus gauge tagged with the environment.
fn publish_gauge(name: &'static str, environment: &str, value: u64) {
	gauge!(name, "env" => environment.to_owned()).set(count_to_f64(value));
}

/// Convert a `u64` count to the `f64` required by a Prometheus gauge.
#[allow(clippy::as_conversions, clippy::cast_precision_loss)]
const fn count_to_f64(value: u64) -> f64 {
	value as f64
}
