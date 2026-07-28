//! `GitHub` pull request-related API calls.

use crate::github::webhook::events::models::RequestedReviewersInfo;
use crate::AppError;
use octocrab::models::pulls::{PullRequest, Review};
use octocrab::Octocrab;

/// Fetch the full [`PullRequest`] object for a given repository's PR number.
pub async fn fetch(
	client: &Octocrab,
	owner: &str,
	repo: &str,
	pr_number: u64,
) -> Result<PullRequest, AppError> {
	Ok(client.pulls(owner, repo).get(pr_number).await?)
}

/// List all submitted reviews belonging to a pull request.
pub async fn fetch_reviews(
	client: &Octocrab,
	owner: &str,
	repo: &str,
	pr_number: u64,
) -> Result<Vec<Review>, AppError> {
	Ok(client
		.pulls(owner, repo)
		.list_reviews(pr_number)
		.send()
		.await?
		.items)
}

/// Return the logins of all pending requested reviewers of a PR.
pub async fn fetch_requested_reviewers(
	client: &Octocrab,
	owner: &str,
	project: &str,
	pr_number: u64,
) -> Result<Vec<String>, AppError> {
	let wrapper: RequestedReviewersInfo = client
		.get(
			&format!("/repos/{owner}/{project}/pulls/{pr_number}/requested_reviewers"),
			None::<&()>,
		)
		.await?;
	Ok(wrapper.users.into_iter().map(|a| a.login).collect())
}

/// Request a review from a user for a pull request.
pub async fn assign_reviewer(
	client: &Octocrab,
	repository: &str,
	pr_number: u64,
	reviewer: &str,
) -> Result<(), AppError> {
	let (owner, repo) = split_repo(repository)?;
	client
		.pulls(owner, repo)
		.request_reviews(pr_number, vec![reviewer.to_owned()], vec![])
		.await?;
	Ok(())
}

/// Remove a user from the pending requested reviewers of a pull request.
pub async fn unassign_reviewer(
	client: &Octocrab,
	repository: &str,
	pr_number: u64,
	reviewer: &str,
) -> Result<(), AppError> {
	let (owner, repo) = split_repo(repository)?;

	client
		.pulls(owner, repo)
		.remove_requested_reviewers(pr_number, vec![reviewer.to_owned()], vec![])
		.await?;

	Ok(())
}

/// Split a full repository name into its `(owner, project)` parts.
///
/// Returns an [`AppError::Message`] instead of panicking when the input
/// isn't in `owner/project` form, so callers can surface a clean error.
pub fn split_repo(repo: &str) -> Result<(&str, &str), AppError> {
	repo.split_once('/')
		.filter(|(owner, project)| !owner.is_empty() && !project.is_empty())
		.ok_or_else(|| {
			AppError::message(format!(
				"repository must be in owner/project format, got `{repo}`"
			))
		})
}
