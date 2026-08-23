//! Pinned `GitHub` webhook payload models and event routing key.

use octocrab::models::pulls::PullRequest;
use octocrab::models::Author;
use serde::de::IgnoredAny;
use serde::{Deserialize, Deserializer};

/// The `X-GitHub-Event` header value, parsed into a routable enum.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GitHubEvent {
	PullRequest,
	PullRequestReview,
	Push,
	IssueComment,
	Installation,
	Ping,
	Unknown(String),
}

impl From<&str> for GitHubEvent {
	fn from(s: &str) -> Self {
		match s {
			"pull_request" => Self::PullRequest,
			"pull_request_review" => Self::PullRequestReview,
			"push" => Self::Push,
			"issue_comment" => Self::IssueComment,
			"installation" => Self::Installation,
			"ping" => Self::Ping,
			other => Self::Unknown(other.to_owned()),
		}
	}
}

impl GitHubEvent {
	/// Stable string label for metrics and logging.
	pub fn as_str(&self) -> &str {
		match self {
			Self::PullRequest => "pull_request",
			Self::PullRequestReview => "pull_request_review",
			Self::Push => "push",
			Self::IssueComment => "issue_comment",
			Self::Installation => "installation",
			Self::Ping => "ping",
			Self::Unknown(other) => other,
		}
	}
}

/// Minimal `GitHub` user reference.
#[derive(Debug, Clone, Deserialize)]
pub struct GitHubUserInfo {
	pub login: String,
}

impl From<Author> for GitHubUserInfo {
	fn from(author: Author) -> Self {
		Self {
			login: author.login,
		}
	}
}

/// Minimal `GitHub` repository reference.
#[derive(Debug, Clone, Deserialize)]
pub struct RepositoryInfo {
	#[serde(deserialize_with = "deserialize_to_lowercase")]
	pub name: String,
	pub owner: GitHubUserInfo,
}

impl RepositoryInfo {
	/// Repository in `owner/name` form.
	#[must_use]
	pub fn full_name(&self) -> String {
		format!("{}/{}", self.owner.login, self.name)
	}
}

/// Minimal repository branch reference.
#[derive(Debug, Clone, Deserialize)]
pub struct PullRequestRefInfo {
	#[serde(rename = "ref")]
	pub branch: String,
	pub sha: String,
}

/// PR state (open / closed).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueStateInfo {
	Open,
	Closed,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstallationIdInfo(pub u64);

/// Minimum `GitHub` app installation reference.
#[derive(Debug, Clone, Deserialize)]
pub struct InstallationInfo {
	pub id: InstallationIdInfo,
	pub account: GitHubUserInfo,
}

/// Minimum issue reference where `number` is the PR number.
#[derive(Debug, Deserialize)]
pub struct IssueInfo {
	pub number: u64,
	/// Present only when the issue is actually a pull request. We only care
	/// about its presence (to filter out plain issues), not its contents.
	pub pull_request: Option<IgnoredAny>,
}

/// Minimum PR review verdict reference.
#[derive(Debug, Deserialize)]
pub struct ReviewInfo {
	pub state: String,
	pub user: GitHubUserInfo,
}

impl ReviewInfo {
	/// Emoji summarizing the review verdict (approved/changes/comment).
	pub fn verdict_emoji(&self) -> &'static str {
		match self.state.to_lowercase().as_str() {
			"approved" => "✅",
			"changes_requested" => "🛑",
			_ => "💬",
		}
	}
}

/// Minimum requested reviewers for a PR, only need `users`.
#[derive(Deserialize)]
pub struct RequestedReviewersInfo {
	pub(crate) users: Vec<GitHubUserInfo>,
}

/// Minimum pull request reference. Resistant to `Octocrab` model changes.
#[derive(Debug, Clone, Deserialize)]
pub struct PullRequestInfo {
	pub number: u64,
	pub title: String,
	pub state: IssueStateInfo,

	#[serde(default)]
	pub merged: bool,

	pub html_url: String,

	pub user: GitHubUserInfo,

	pub head: PullRequestRefInfo,
	pub base: PullRequestRefInfo,

	#[serde(default)]
	pub additions: u64,
	#[serde(default)]
	pub deletions: u64,
	#[serde(default)]
	pub changed_files: u64,
	#[serde(default)]
	pub commits: u64,
	#[serde(default)]
	pub comments: u64,
	#[serde(default)]
	pub review_comments: u64,

	#[serde(default)]
	pub requested_reviewers: Vec<GitHubUserInfo>,
}

impl From<PullRequest> for PullRequestInfo {
	fn from(pr: PullRequest) -> Self {
		let state = match pr.state {
			Some(octocrab::models::IssueState::Closed) => IssueStateInfo::Closed,
			_ => IssueStateInfo::Open,
		};

		Self {
			number: pr.number,
			title: pr.title.unwrap_or_default(),
			state,
			merged: pr.merged.unwrap_or(false),
			html_url: pr.html_url.map(|u| u.to_string()).unwrap_or_default(),
			user: GitHubUserInfo {
				login: pr.user.map(|u| u.login).unwrap_or_default(),
			},
			head: PullRequestRefInfo {
				branch: pr.head.ref_field.clone(),
				sha: pr.head.sha.clone(),
			},
			base: PullRequestRefInfo {
				branch: pr.base.ref_field.clone(),
				sha: pr.base.sha.clone(),
			},
			additions: pr.additions.unwrap_or(0),
			deletions: pr.deletions.unwrap_or(0),
			changed_files: pr.changed_files.unwrap_or(0),
			commits: pr.commits.unwrap_or(0),
			comments: pr.comments.unwrap_or(0),
			review_comments: pr.review_comments.unwrap_or(0),
			requested_reviewers: pr
				.requested_reviewers
				.unwrap_or_default()
				.into_iter()
				.map(GitHubUserInfo::from)
				.collect(),
		}
	}
}

/// Extension trait adding PR status presentation helpers to [`PullRequestInfo`].
pub trait PullRequestExt {
	/// Emoji summarizing the PR's lifecycle state (merged/closed/open).
	fn status_emoji(&self) -> &'static str;
}

impl PullRequestExt for PullRequestInfo {
	fn status_emoji(&self) -> &'static str {
		match (&self.state, self.merged) {
			(IssueStateInfo::Closed, true) => "🟣",
			(IssueStateInfo::Closed, _) => "🔴",
			_ => "🟢",
		}
	}
}

/// Custom deserializer that lowercases the input string.
fn deserialize_to_lowercase<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
	String::deserialize(d).map(|s| s.to_lowercase())
}
