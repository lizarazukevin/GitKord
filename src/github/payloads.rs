//! GitHub webhook payload types.
//!
//! Each struct maps to the JSON response GitHub sends for a given event.
//! If it has a deserializable tag and produced by GitHub it belongs here,
//! not consumed by other parts of our project. Unkown fields are silently
//! ignored so new GitHub fields do not break deserialization.

use serde::Deserialize;

/// The `X-GitHub-Event` header, identifies event type delivered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitHubEvent {
    PullRequest,
    PullRequestReview,
    Push,
    IssueComment,

    /// Sent by GitHub when webhook is first registered.
    Ping,

    /// Catch-all event type we do not handle yet.
    Unknown(String),
}

impl From<&str> for GitHubEvent {
    fn from(s: &str) -> Self {
        match s {
            "pull_request" => Self::PullRequest,
            "pull_request_review" => Self::PullRequestReview,
            "push" => Self::Push,
            "issue_comment" => Self::IssueComment,
            "ping" => Self::Ping,
            other => Self::Unknown(other.to_owned()),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct PullRequestPayload {
    pub action: String,
    pub pull_request: PullRequest,
    pub repository: Repository,
}

#[derive(Debug, Deserialize)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub merged: Option<bool>,
    pub html_url: String,
    pub user: GitHubUser,
    pub head: PullRequestRef,
    pub base: PullRequestRef,
}

#[derive(Debug, Deserialize)]
pub struct Repository {
    pub full_name: String,
}

impl PullRequest {
    pub fn status_label(&self) -> &'static str {
        match (self.state.as_str(), self.merged) {
            ("closed", Some(true)) => "Merged",
            ("closed", _) => "Closed",
            _ => "Open",
        }
    }

    pub fn status_emoji(&self) -> &'static str {
        match (self.state.as_str(), self.merged) {
            ("closed", Some(true)) => "🟣",
            ("closed", _) => "🔴",
            _ => "🟢",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct GitHubUser {
    pub login: String,

    #[expect(dead_code)]
    pub id: u64,
}

#[derive(Debug, Deserialize)]
pub struct PullRequestRef {
    #[serde(rename = "ref")]
    pub branch: String,

    #[expect(dead_code)]
    pub sha: String,
}

#[derive(Debug, Deserialize)]
pub struct PullRequestReviewPayload {
    pub action: String,
    pub review: Review,
    pub pull_request: PullRequest,
    pub repository: Repository,
}

#[derive(Debug, Deserialize)]
pub struct Review {
    pub state: String,
    pub user: GitHubUser,

    #[expect(dead_code)]
    pub body: Option<String>,
}

impl Review {
    pub fn verdict_emoji(&self) -> &'static str {
        match self.state.to_lowercase().as_str() {
            "approved" => "✅",
            "changes_requested" => "🛑",
            _ => "💬",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct PushPayload {
    #[serde(rename = "ref")]
    pub git_ref: String,
    pub after: String,
    pub repository: Repository,
    pub commits: Vec<Commit>,
}

#[derive(Debug, Deserialize)]
pub struct Commit {
    #[expect(dead_code)]
    pub id: String,

    #[expect(dead_code)]
    pub message: String,

    #[expect(dead_code)]
    pub author: CommitAuthor,
}

#[derive(Debug, Deserialize)]
pub struct CommitAuthor {
    #[expect(dead_code)]
    pub name: String,

    #[expect(dead_code)]
    pub email: String,

    /// GitHub username (only present in push payloads)
    #[expect(dead_code)]
    pub username: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct IssueCommentPayload {
    pub action: String,
    pub issue: Issue,
    pub repository: Repository,
}

#[derive(Debug, Deserialize)]
pub struct Issue {
    pub number: u64,
    pub pull_request: Option<IssuePullRequest>,
}

#[derive(Debug, Deserialize)]
pub struct IssuePullRequest {
    #[allow(dead_code)]
    pub url: String,
}
