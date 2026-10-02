//! Domain model and traits for collected email signups.

use crate::AppError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use email_address::EmailAddress;
use serde::Serialize;
use std::str::FromStr;

const MAX_EMAIL_LENGTH: usize = 320;

/// Returned when input is not in the right email format.
/// No info, meant to return an answer with generic 400.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("invalid email address")]
pub struct InvalidEmail;

impl From<InvalidEmail> for AppError {
	fn from(e: InvalidEmail) -> Self {
		Self::Message(e.to_string())
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Email(String);

impl Email {
	pub fn parse(raw: &str) -> Result<Self, InvalidEmail> {
		if raw.len() > MAX_EMAIL_LENGTH {
			return Err(InvalidEmail);
		}

		let s = raw.trim().to_ascii_lowercase();
		if !s.is_ascii() {
			return Err(InvalidEmail);
		}

		let addr = EmailAddress::from_str(&s).map_err(|_| InvalidEmail)?;

		if !addr.domain().contains('.') {
			return Err(InvalidEmail);
		}

		Ok(Self(s))
	}

	pub fn as_str(&self) -> &str {
		&self.0
	}
}

#[derive(Debug, Clone, Serialize)]
pub struct NewsletterSignup {
	pub id: i64,
	pub email: String,
	pub subscribed: bool,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

#[async_trait]
pub trait NewsletterSignupStore: Send + Sync {
	/// Insert an email or re-subscribe if it exists.
	async fn upsert(&self, email: &Email) -> Result<(), AppError>;

	/// Mark an email as opted out.
	async fn unsubscribe(&self, email: &Email) -> Result<(), AppError>;
	/// Oldest first, keyset-paginated by `id`.
	/// None = all, Some(true) = opted in, Some(false) = opted out
	async fn fetch_all_newsletter_signup_emails(
		&self,
		max_results: u32,
		next_token: Option<i64>,
		is_subscribed: Option<bool>,
	) -> Result<Vec<NewsletterSignup>, AppError>;
}
