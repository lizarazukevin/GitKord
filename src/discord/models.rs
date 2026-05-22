//! Domain models for discord messages sent by `GitKord`.

use crate::discord::context::AppState;

/// Event handler stored in the Serenity client.
pub struct ReadyHandler {
    pub app_state: AppState,
}

pub struct PostedPullRequest {
    pub message_id: u64,
    pub thread_id: u64,
}

pub struct ReviewerRequest {
    pub owner: String,
    pub repo_name: String,
    pub repo: String,
    pub pr_number: u64,
    pub github_login: String,
}

pub enum ReviewerAction {
    Assign,
    Unassign,
}
