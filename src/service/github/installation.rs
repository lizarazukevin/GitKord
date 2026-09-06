//! Service layer for `GitHub` App installation events.

use crate::error::AppError;
use crate::github::api::pull_requests::split_repo;
use crate::github::webhook::events::installation::InstallationPayload;
use crate::models::subscription::SubscriptionStore;
use std::sync::Arc;
use tracing::{error, info};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallationAction {
	Created,
	Deleted,
	Other,
}

pub struct InstallationRequest {
	pub action: InstallationAction,
	pub repositories: Vec<String>,
}

impl InstallationRequest {
	pub(crate) fn from_payload(payload: InstallationPayload) -> Self {
		Self {
			action: match payload.action.as_str() {
				"created" => InstallationAction::Created,
				"deleted" => InstallationAction::Deleted,
				_ => InstallationAction::Other,
			},
			repositories: payload.repositories.into_iter().collect(),
		}
	}
}

pub struct InstallationService {
	sub_store: Arc<dyn SubscriptionStore>,
}

impl InstallationService {
	pub fn new(sub_store: Arc<dyn SubscriptionStore>) -> Self {
		Self { sub_store }
	}

	/// React to an installation lifecycle event.
	///
	/// For created: logs the installation.
	/// For deleted : removes all subscriptions for every repository belonging to the installation.
	pub async fn handle(&self, req: InstallationRequest) -> Result<(), AppError> {
		match req.action {
			InstallationAction::Created => {
				info!("app installed");
			}
			InstallationAction::Deleted => {
				info!("app uninstalled, cleaning up subscriptions");

				for repo in &req.repositories {
					let (owner, project) = split_repo(repo)?;
					if let Err(e) = self
						.sub_store
						.delete_all_by_owner_project(owner, project)
						.await
					{
						error!(
							error = %e,
							repo = %repo,
							"failed to clean up subscriptions"
						);
					}
				}
			}
			InstallationAction::Other => {}
		}

		Ok(())
	}
}
