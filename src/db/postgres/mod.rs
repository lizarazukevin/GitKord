//! `Postgres` implementations and schemas for data storage.

use crate::db::postgres::migrate::run_migrations;
use crate::db::postgres::pr_messages::PgPrMessageStore;
use crate::db::postgres::subscriptions::PgSubscriptionStore;
use crate::db::postgres::user_links::PgUserStore;
use crate::error::AppError;
use crate::models::pr_message::PrStore;
use crate::models::subscription::SubscriptionStore;
use crate::models::user_link::UserStore;
use sqlx::PgPool;
use std::sync::Arc;

mod migrate;
mod pr_messages;
mod subscriptions;
mod user_links;

pub struct Stores {
	pub(crate) prs: Arc<dyn PrStore>,
	pub(crate) subscriptions: Arc<dyn SubscriptionStore>,
	pub(crate) users: Arc<dyn UserStore>,
}

/// Create the three `Postgres`‑backed stores, ready for injection.
///
/// Opens a connection pool, runs schema migrations, and returns
/// the stores wrapped in `Arc` so they can be shared across the
/// application. Call once at startup.
pub async fn create_stores(database_url: &str) -> Result<Stores, AppError> {
	let pool = PgPool::connect(database_url).await?;

	run_migrations(&pool).await?;

	Ok(Stores {
		prs: Arc::new(PgPrMessageStore::new(pool.clone())),
		subscriptions: Arc::new(PgSubscriptionStore::new(pool.clone())),
		users: Arc::new(PgUserStore::new(pool)),
	})
}
