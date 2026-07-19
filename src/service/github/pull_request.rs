//! Service layer for pull‑request events.

use serenity::all::{ChannelId, Http};
use std::sync::Arc;
use tracing::error;

use crate::discord::messaging::audit::{post_commit_push, post_lifecycle_pr_update};
use crate::discord::messaging::messages::post_pull_request_message;
use crate::error::AppError;
use crate::github::api::client::GitHubClient;
use crate::github::webhook::events::models::PullRequestInfo;
use crate::github::webhook::events::pull_request::PullRequestPayload;
use crate::models::pr_message::{PrMessage, PrStore};
use crate::models::subscription::SubscriptionStore;
use crate::models::user_link::UserStore;
use crate::service::github::pr_messages::{load_pr_message_data, update_all_pr_messages};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullRequestAction {
    Opened,
    Closed,
    Reopened,
    Synchronize,
    Edited,
    ReviewRequested,
    ReviewRequestRemoved,
    Other,
}

impl PullRequestAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Opened => "opened",
            Self::Closed => "closed",
            Self::Reopened => "reopened",
            Self::Synchronize => "synchronize",
            Self::Edited => "edited",
            Self::ReviewRequested => "review_requested",
            Self::ReviewRequestRemoved => "review_request_removed",
            Self::Other => "other",
        }
    }
}

pub struct PullRequestRequest {
    pub action: PullRequestAction,
    pub owner: String,
    pub project: String,
    pub pr: PullRequestInfo,
    pub sender: String,
}

impl PullRequestRequest {
    pub fn from_payload(payload: PullRequestPayload) -> Self {
        Self {
            action: match payload.action.as_str() {
                "opened" => PullRequestAction::Opened,
                "closed" => PullRequestAction::Closed,
                "reopened" => PullRequestAction::Reopened,
                "synchronize" => PullRequestAction::Synchronize,
                "edited" => PullRequestAction::Edited,
                "review_requested" => PullRequestAction::ReviewRequested,
                "review_request_removed" => PullRequestAction::ReviewRequestRemoved,
                _ => PullRequestAction::Other,
            },
            owner: payload.repository.owner.login,
            project: payload.repository.name,
            pr: payload.pull_request,
            sender: payload.sender.login,
        }
    }
}

pub struct PullRequestService {
    pr_store: Arc<dyn PrStore>,
    sub_store: Arc<dyn SubscriptionStore>,
    user_store: Arc<dyn UserStore>,
    github: Arc<GitHubClient>,
    http: Arc<Http>,
}

impl PullRequestService {
    pub fn new(
        pr_store: Arc<dyn PrStore>,
        sub_store: Arc<dyn SubscriptionStore>,
        user_store: Arc<dyn UserStore>,
        github: Arc<GitHubClient>,
        http: Arc<Http>,
    ) -> Self {
        Self {
            pr_store,
            sub_store,
            user_store,
            github,
            http,
        }
    }

    /// Route a PR event to the handler for its action.
    pub async fn handle(&self, req: PullRequestRequest) -> Result<(), AppError> {
        match req.action {
            PullRequestAction::Opened => self.handle_opened(req).await,
            PullRequestAction::Synchronize => self.handle_synchronize(req).await,
            PullRequestAction::Closed | PullRequestAction::Reopened => {
                self.handle_lifecycle_change(req).await
            }
            PullRequestAction::Edited
            | PullRequestAction::ReviewRequested
            | PullRequestAction::ReviewRequestRemoved => self.refresh_and_skip_audit(req).await,
            PullRequestAction::Other => Ok(()),
        }
    }

    /// A new PR was opened, post the initial message to every subscribed channel.
    /// Side effect of [post_pull_request_message] is creating an audit thread.
    async fn handle_opened(&self, req: PullRequestRequest) -> Result<(), AppError> {
        let repository = format!("{}/{}", req.owner, req.project);

        let message_data = load_pr_message_data(
            &self.github,
            self.sub_store.as_ref(),
            self.user_store.as_ref(),
            &req.owner,
            &req.project,
            req.pr.number,
        )
        .await?;

        let subscriptions = self.sub_store.fetch_all_by_repo(&repository).await?;
        for sub in &subscriptions {
            let posted = post_pull_request_message(
                &self.http,
                ChannelId::from(sub.channel_id),
                &message_data,
            )
            .await?;

            self.pr_store
                .upsert(PrMessage {
                    repository: repository.clone(),
                    pr: req.pr.number,
                    channel_id: sub.channel_id,
                    message_id: posted.message_id,
                    thread_id: posted.thread_id,
                })
                .await?;
        }

        Ok(())
    }

    /// New commits were pushed. Refresh the main message and append a commit‑push
    /// audit entry to every thread.
    async fn handle_synchronize(&self, req: PullRequestRequest) -> Result<(), AppError> {
        let pr_messages = self
            .update_pr_messages(&req.owner, &req.project, req.pr.number)
            .await?;

        let sha = req.pr.head.sha;
        for msg in &pr_messages {
            if let Err(e) = post_commit_push(&self.http, msg.thread_id, &req.sender, &sha).await {
                error!(error = %e, thread_id = msg.thread_id, "failed to post commit push");
            }
        }

        Ok(())
    }

    /// The PR was closed or reopened. Refresh the main message and append a
    /// lifecycle audit entry.
    async fn handle_lifecycle_change(&self, req: PullRequestRequest) -> Result<(), AppError> {
        let pr_messages = self
            .update_pr_messages(&req.owner, &req.project, req.pr.number)
            .await?;

        for message in &pr_messages {
            if let Err(e) = post_lifecycle_pr_update(
                &self.http,
                message.thread_id,
                req.pr.number,
                req.action.as_str(),
                req.pr.merged,
            )
            .await
            {
                error!(error = %e, thread_id = message.thread_id, "failed to post lifecycle audit");
            }
        }

        Ok(())
    }

    /// Actions that only change the visible PR data (edited, review requests).
    ///
    /// Refresh the message without any audit entry. Only `Discord` commands that
    /// target `/assign` or `/unassign` post an audit entry, this reduces noise.
    async fn refresh_and_skip_audit(&self, req: PullRequestRequest) -> Result<(), AppError> {
        self.update_pr_messages(&req.owner, &req.project, req.pr.number)
            .await?;
        Ok(())
    }

    /// Refresh the PR status message in every subscribed channel.
    /// Returns the list of PR message records that were updated.
    ///
    /// Delegates the fetch-and-edit to [`update_all_pr_messages`] so the
    /// update loop lives in exactly one place.
    async fn update_pr_messages(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<Vec<PrMessage>, AppError> {
        let repository = format!("{owner}/{repo}");

        let message_data = load_pr_message_data(
            &self.github,
            self.sub_store.as_ref(),
            self.user_store.as_ref(),
            owner,
            repo,
            pr_number,
        )
        .await?;

        update_all_pr_messages(
            self.pr_store.as_ref(),
            &self.http,
            &repository,
            pr_number,
            &message_data,
        )
        .await
    }
}
