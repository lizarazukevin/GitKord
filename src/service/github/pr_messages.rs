//! Full pull request data required to construct the `Discord` messages seen by users.
//!
//! Responsible for constructing, loading, and sending updates made to PRs triggered
//! by `GitHub` webhook events.

use crate::discord::messaging::messages::update_pull_request_message;
use crate::error::AppError;
use crate::github::api;
use crate::github::api::client::GitHubClient;
use crate::github::webhook::events::models::{GitHubUserInfo, PullRequestExt, PullRequestInfo};
use crate::models::pr_message::{PrMessage, PrStore};
use crate::models::subscription::SubscriptionStore;
use crate::models::user_link::UserStore;
use indexmap::IndexMap;
use octocrab::models::pulls::{Review, ReviewState};
use octocrab::Octocrab;
use serenity::all::{ChannelId, Http};

pub struct PrMessageData {
    pub status_emoji: &'static str,
    pub number: u64,
    pub title: String,
    pub author: String,
    pub repository: String,
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

#[allow(unused)]
pub enum CheckStatus {
    Success,
    Failure,
    Pending,
}

impl PrMessageData {
    pub fn new(
        owner: &str,
        project: &str,
        pr: &PullRequestInfo,
        reviewers: Vec<ReviewSummary>,
    ) -> Self {
        PrMessageData {
            status_emoji: pr.status_emoji(),
            number: pr.number,
            title: pr.title.clone(),
            author: pr.user.login.clone(),
            repository: format!("{owner}/{project}"),
            head: pr.head.branch.clone(),
            base: pr.base.branch.clone(),
            url: pr.html_url.to_string(),
            additions: pr.additions,
            deletions: pr.deletions,
            files: pr.changed_files,
            commits: pr.commits,
            comments: pr.comments + pr.review_comments,
            reviews: reviewers,
            checks: vec![],
        }
    }
}

pub async fn load_pr_message_data(
    github: &GitHubClient,
    sub_store: &dyn SubscriptionStore,
    user_store: &dyn UserStore,
    owner: &str,
    project: &str,
    pr_number: u64,
) -> Result<PrMessageData, AppError> {
    let gh_install_client = installation_client_for_repo(sub_store, github, owner, project).await?;

    let pr = PullRequestInfo::from(
        api::pull_requests::fetch(&gh_install_client, owner, project, pr_number).await?,
    );
    let submitted_reviews =
        api::pull_requests::fetch_reviews(&gh_install_client, owner, project, pr_number).await?;

    let reviewer_states = merge_review_states(&submitted_reviews, &pr.requested_reviewers);
    let reviewers = enrich_reviewers(user_store, reviewer_states).await;

    Ok(PrMessageData::new(owner, project, &pr, reviewers))
}

async fn installation_client_for_repo(
    sub_store: &dyn SubscriptionStore,
    github: &GitHubClient,
    owner: &str,
    project: &str,
) -> Result<Octocrab, AppError> {
    let repository = format!("{owner}/{project}");
    let id = sub_store.fetch_installation_id_by_repo(&repository).await?;
    github.scoped_to_installation(id)
}

/// Merge submitted reviews with requested reviewers.
fn merge_review_states(
    submitted_reviews: &[Review],
    requested_reviewers: &[GitHubUserInfo],
) -> IndexMap<String, ReviewState> {
    let mut states = IndexMap::new();

    for review in submitted_reviews {
        if let Some(user) = &review.user {
            let login = user.login.clone();
            if !login.is_empty() {
                let state = review.state.unwrap_or(ReviewState::Commented);
                states.insert(login, state);
            }
        }
    }

    for reviewer in requested_reviewers {
        states
            .entry(reviewer.login.clone())
            .or_insert(ReviewState::Pending);
    }

    states
}

/// Displays a user's `Discord` tag when a link to their `GitHub` login is found.
async fn enrich_reviewers(
    user_store: &dyn UserStore,
    reviewers: IndexMap<String, ReviewState>,
) -> Vec<ReviewSummary> {
    let logins: Vec<String> = reviewers.keys().cloned().collect();
    let discord_map = user_store
        .fetch_by_github_logins(&logins)
        .await
        .unwrap_or_default();

    reviewers
        .into_iter()
        .map(|(github_login, state)| {
            let discord_tag = discord_map
                .get(&github_login)
                .map(|id| format!("<@{}>", id));
            ReviewSummary {
                github_login,
                discord_tag,
                state,
            }
        })
        .collect()
}

/// Updates stored PR messages across all subscribed channels with new webhook activity in place.
///
/// A failure updating one channel aborts the batch and propagates; callers
/// that need best-effort delivery should handle that at their level.
/// Returns the list of message records that were targeted.
pub async fn update_all_pr_messages(
    pr_store: &dyn PrStore,
    http: &Http,
    repository: &str,
    pr_number: u64,
    message_data: &PrMessageData,
) -> Result<Vec<PrMessage>, AppError> {
    let pr_messages = pr_store
        .fetch_all_by_repo_and_pr(repository, pr_number)
        .await?;
    for msg in &pr_messages {
        update_pull_request_message(
            http,
            ChannelId::from(msg.channel_id),
            msg.message_id,
            message_data,
        )
        .await?;
    }
    Ok(pr_messages)
}
