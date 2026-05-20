//! Domain models owned by `DiGiBot`.
//!
//! These are distinct from `payloads.rs` which mirrors GitHub's JSON shapes.
//! Types here are produced by `api.rs` and consumed by `messages.rs`.

pub struct PrMessageData {
    pub status_emoji: &'static str,
    pub number: u64,
    pub title: String,
    pub author: String,
    pub repo: String,
    pub head: String,
    pub base: String,
    pub url: String,
    pub additions: u64,
    pub deletions: u64,
    pub files: u64,
    pub commits: u64,
    pub comments: u64,
    pub reviews: Vec<ReviewSummary>,
    pub checks: Vec<CheckSummary>,
}

pub struct ReviewSummary {
    pub github_login: String,
    pub discord_tag: Option<String>,
    pub state: ReviewState,
}

pub struct CheckSummary {
    pub name: String,
    pub conclusion: CheckStatus,
}

pub enum ReviewState {
    Approved,
    ChangesRequested,
    Commented,
    Pending,
}

#[expect(dead_code)]
pub enum CheckStatus {
    Success,
    Failure,
    Pending,
}
