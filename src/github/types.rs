#![expect(dead_code)]

//! GitHub webhook payload types.
//!
//! Each struct maps to the JSON response GitHub sends.
//! Fields are `Option` where conditionally present by the docs.

use serde::Deserialize;

/// The `X-GitHub-Event` header, identifies which event type was delivered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitHubEvent {
    /// Pull request was opened, closed, edited, etc.
    PullRequest,

    /// A review was submitted, dismissed, or edited
    PullRequestReview,

    /// Commits pushed to a branch
    Push,

    /// GitHub's connectivity test
    Ping,

    /// Catch-all event type handler does not exist
    Unknown(String),
}

impl From<&str> for GitHubEvent {
    fn from(s: &str) -> Self {
        match s {
            "pull_request" => Self::PullRequest,
            "pull_request_review" => Self::PullRequestReview,
            "push" => Self::Push,
            "ping" => Self::Ping,
            other => Self::Unknown(other.to_owned()),
        }
    }
}

// — Pull Request Event ───────────────────────────────────────

/// Payload for `pull_request` webhook event.
#[derive(Debug, Deserialize)]
pub struct PullRequestPayload {
    /// What occurred (e.g. "opened", "closed")
    pub action: String,

    /// Contents of the pull request
    pub pull_request: PullRequest,

    /// Owning repository behind the PR
    pub repository: Repository,
}

/// A GitHub pull request.
#[derive(Debug, Deserialize)]
pub struct PullRequest {
    /// Numeric PR identifier (e.g. `42`)
    pub number: u64,

    /// PR title.
    pub title: String,

    /// Current state: `"open"` or `"closed"`
    pub state: String,

    /// Whether the PR was merged (only meaningful when `state == "closed"`)
    pub merged: Option<bool>,

    /// URL to open the PR in a browser
    pub html_url: String,

    /// The user who opened the PR
    pub user: GitHubUser,

    /// The most recent commit SHA on the PR branch
    pub head: PullRequestRef,

    /// The branch the PR targets
    pub base: PullRequestRef,
}

impl PullRequest {
    /// Returns a human-readable status label for use in Discord messages
    pub fn status_label(&self) -> &'static str {
        match (self.state.as_str(), self.merged) {
            ("closed", Some(true)) => "Merged",
            ("closed", _) => "Closed",
            _ => "Open",
        }
    }

    /// Returns the status emoji for this PR's current state
    pub fn status_emoji(&self) -> &'static str {
        match (self.state.as_str(), self.merged) {
            ("closed", Some(true)) => "🟣",
            ("closed", _) => "🔴",
            _ => "🟢",
        }
    }
}

/// A git ref (branch + commit) on one side of the pull request
#[derive(Debug, Deserialize)]
pub struct PullRequestRef {
    /// Branch name (e.g. `"main"` or `"feat/my-feature"`)
    #[serde(rename = "ref")]
    pub branch: String,

    /// The commit SHA at the tip of this ref
    pub sha: String,
}

// — Pull Request Review Event ────────────────────────────────

/// Payload for `pull_request_review` webhook event.
#[derive(Debug, Deserialize)]
pub struct PullRequestReviewPayload {
    /// What occurred (e.g. "submitted", "edited")
    pub action: String,

    /// The review that was submitted
    pub review: Review,

    /// The PR the review was left on
    pub pull_request: PullRequest,

    /// The repository owning this PR
    pub repository: Repository,
}

/// A single pull request review.
#[derive(Debug, Deserialize)]
pub struct Review {
    /// Review verdict (e.g. "approved", "commented")
    pub state: String,

    /// The reviewer attached to this event
    pub user: GitHubUser,

    /// Optional review body text
    pub body: Option<String>,
}

impl Review {
    /// Returns the emoji representing this review's verdict
    pub fn verdict_emoji(&self) -> &'static str {
        match self.state.to_lowercase().as_str() {
            "approved" => "✅",
            "changes_requested" => "❌",
            _ => "💬",
        }
    }
}

// — Push Event ───────────────────────────────────────────────

/// Payload for a `push` webhook event.
#[derive(Debug, Deserialize)]
pub struct PushPayload {
    #[serde(rename = "ref")]
    pub git_ref: String,
    pub after: String,
    pub repository: Repository,
    pub commits: Vec<Commit>,
}

/// Single commit in a push payload.
#[derive(Debug, Deserialize)]
pub struct Commit {
    /// Commit SHA
    pub id: String,

    /// First line of the commit message
    pub message: String,

    /// The owner of the commit
    pub author: CommitAuthor,
}

/// Authorship metadata on a commit.
#[derive(Debug, Deserialize)]
pub struct CommitAuthor {
    /// Display name
    pub name: String,

    /// Email address
    pub email: String,

    /// GitHub username (only present in push payloads)
    #[serde(default)]
    pub username: Option<String>,
}

// — Shared Types ─────────────────────────────────────────────

/// A GitHub user as it appears on the webhook payload.
#[derive(Debug, Deserialize)]
pub struct GitHubUser {
    /// GitHub username
    pub login: String,

    /// Numeric user ID
    pub id: u64,
}

/// A repository as it appears in webhook payloads.
#[derive(Debug, Deserialize)]
pub struct Repository {
    /// Full name in `owner/name` format (e.g. "lizarazukevin/DiGiBot")
    pub full_name: String,
}
