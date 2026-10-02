//! Service layer for the newsletter: signups, opt-outs, and listing.
//!
//! Owns the signup store and will grow to dispatch email updates.

use crate::models::newsletter_signup::{Email, NewsletterSignup, NewsletterSignupStore};
use crate::AppError;
use serde::Deserialize;
use std::sync::Arc;

const DEFAULT_PAGE_SIZE: u32 = 100;

/// Shared request body for the public signup and admin unsubscribe endpoints.
#[derive(Deserialize)]
pub struct NewsletterEmailRequest {
	pub email: String,
}

pub struct NewsletterService {
	store: Arc<dyn NewsletterSignupStore>,
}

impl NewsletterService {
	pub fn new(store: Arc<dyn NewsletterSignupStore>) -> Self {
		Self { store }
	}

	/// Subscribe an email, or re-subscribe it if it already exists (upsert).
	pub async fn subscribe(&self, email: &str) -> Result<(), AppError> {
		let parsed = Email::parse(email)?;
		self.store.upsert(&parsed).await
	}

	/// Opt an email out; no-op if it isn't currently subscribed.
	pub async fn unsubscribe(&self, email: &str) -> Result<(), AppError> {
		let parsed = Email::parse(email)?;
		self.store.unsubscribe(&parsed).await
	}

	/// List signups (oldest first, keyset-paginated).
	/// None = all, Some(true) = opted in, Some(false) = opted out.
	pub async fn list(
		&self,
		max_results: Option<u32>,
		next_token: Option<i64>,
		is_subscribed: Option<bool>,
	) -> Result<Vec<NewsletterSignup>, AppError> {
		self.store
			.fetch_all_newsletter_signup_emails(
				max_results.unwrap_or(DEFAULT_PAGE_SIZE),
				next_token,
				is_subscribed,
			)
			.await
	}
}
