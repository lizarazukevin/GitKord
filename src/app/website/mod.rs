//! Public-facing website endpoints.

use crate::models::newsletter_signup::NewsletterSignupStore;
use axum::Router;
use std::sync::Arc;

pub mod newsletter;

pub fn router(store: Arc<dyn NewsletterSignupStore>) -> Router {
	Router::new().merge(newsletter::routes()).with_state(store)
}
