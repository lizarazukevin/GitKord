//! HMAC-SHA256 signature verification for incoming `GitHub` webhook payloads.
//!
//! `GitHub` signs every webhook body with the shared secret configured on
//! the App/repository; this module recomputes that signature and rejects the
//! request if it doesn't match.

use crate::error::AppError;
use axum::body::Bytes;
use hmac::{Hmac, KeyInit, Mac};
use http::HeaderMap;
use sha2::Sha256;
use tracing::log::warn;

pub type HmacSha256 = Hmac<Sha256>;

/// Verifies a webhook request's `X-Hub-Signature-256` header against its
/// raw body, using the secret configured for this `GitHub` App/webhook.
pub struct WebhookVerifier {
    secret: String,
}

impl WebhookVerifier {
    pub fn new(secret: String) -> Self {
        Self { secret }
    }

    pub fn verify(&self, headers: &HeaderMap, body: &Bytes) -> Result<(), AppError> {
        let signature = Self::extract_signature(headers)?;
        let expected_mac = self.compute_mac(body);

        expected_mac.verify_slice(&signature).map_err(|_| {
            warn!("webhook signature mismatch, request rejected");
            AppError::InvalidSignature
        })
    }

    /// Pulls the raw signature bytes out of the `X-Hub-Signature-256`
    /// header, stripping `GitHub`'s `sha256=` prefix and hex-decoding.
    fn extract_signature(headers: &HeaderMap) -> Result<Vec<u8>, AppError> {
        let header_value = headers
            .get("x-hub-signature-256")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("sha256="))
            .ok_or(AppError::InvalidSignature)?;

        hex::decode(header_value).map_err(|_| AppError::InvalidSignature)
    }

    /// Builds the HMAC-SHA256 instance for `body`, keyed with this
    /// verifier's secret. Keying an HMAC never fails regardless of key
    /// length (HMAC pads or hashes the key as needed per RFC 2104), so
    /// this doesn't need to return a `Result`.
    fn compute_mac(&self, body: &Bytes) -> Box<HmacSha256> {
        let mut mac: HmacSha256 = KeyInit::new_from_slice(self.secret.as_bytes())
            .expect("HMAC accepts keys of any length");
        mac.update(body);
        Box::new(mac)
    }
}
