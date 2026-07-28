//! `GitHub` webhook-related API calls.

use crate::AppError;
use octocrab::models::hooks::{Config as HookConfig, ContentType as HookContentType, Hook};
use octocrab::models::webhook_events::WebhookEventType;
use octocrab::Octocrab;
use tracing::info;

/// Register a webhook on a repository.
///
/// Returns the hook ID if one was created, `None` if it already existed (HTTP 422).
/// Used in local dev mode to point webhooks at the local ngrok URL.
/// In production, the `GitHub` App handles webhook delivery automatically.
pub async fn register(
	client: &Octocrab,
	owner: &str,
	project: &str,
	payload_url: &str,
	secret: &str,
) -> Result<Option<u64>, AppError> {
	let config = HookConfig {
		url: payload_url.to_owned(),
		content_type: Some(HookContentType::Json),
		secret: Some(secret.to_owned()),
		insecure_ssl: None,
	};

	let hook = Hook {
		name: "web".to_owned(),
		config,
		events: vec![
			WebhookEventType::PullRequest,
			WebhookEventType::PullRequestReview,
			WebhookEventType::Push,
			WebhookEventType::IssueComment,
		],
		active: true,
		..Hook::default()
	};

	match client.repos(owner, project).create_hook(hook).await {
		Ok(created) => {
			let hook_id = created.id;
			info!(owner, project, hook_id, "webhook registered");
			Ok(Some(hook_id))
		}

		Err(octocrab::Error::GitHub { source, .. }) if source.status_code == 422 => {
			info!(owner, project, "webhook already registered, skipping");
			Ok(None)
		}
		Err(e) => Err(AppError::GitHub(e)),
	}
}
