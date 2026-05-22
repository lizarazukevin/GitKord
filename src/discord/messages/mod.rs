//! Discord message handling — formatting, posting, and audit trail.
//!
//! Split into three focused modules:
//! - renderer  — pure formatting, no I/O
//! - transport — Discord API calls for posting and editing
//! - audit     — audit thread entries for PR lifecycle events

pub mod audit;
pub mod renderer;
pub mod transport;

pub use audit::{post_pr_update, post_review, post_reviewer_change};
pub use transport::{post_pull_request_message, update_pull_request_message};
