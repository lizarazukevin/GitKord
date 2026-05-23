//! Link and unlink command handlers.
//!
//! Manages the mapping between Discord user IDs and GitHub usernames.
//! Linked users are @mentioned in PR messages instead of showing their
//! GitHub login, and can be used as reviewer targets via Discord mention.

use crate::discord::commands::shared::{ephemeral, string_option};
use crate::discord::context::AppState;
use crate::github::api;
use crate::state::models::UserLink;
use serenity::all::{CommandInteraction, Context};
use tracing::info;

/// Link the invoking Discord user to a GitHub username.
///
/// Verifies the GitHub username exists before storing the link.
/// Subsequent PR messages will @mention this user when their GitHub
/// login appears as a reviewer.
pub async fn handle_link(
    ctx: &Context,
    cmd: &CommandInteraction,
    app_state: &AppState,
) -> Result<(), serenity::Error> {
    let github_login = string_option(cmd, "username");
    if github_login.is_empty() {
        return ephemeral(ctx, cmd, "Provide your GitHub username.").await;
    }

    match api::verify_user(&app_state.github, &github_login).await {
        Ok(Some(verified)) => {
            let discord_id = cmd.user.id.get();
            match app_state
                .user_store
                .upsert(UserLink {
                    discord_id,
                    github_login: verified.clone(),
                })
                .await
            {
                Ok(()) => {
                    info!(discord_id, github_login = %verified, "user link saved");
                    ephemeral(
                        ctx,
                        cmd,
                        &format!("Linked your Discord account to **{verified}** on GitHub."),
                    )
                    .await
                }
                Err(e) => {
                    tracing::error!(error = %e, "failed to save user link");
                    ephemeral(
                        ctx,
                        cmd,
                        "Something went wrong saving your link. Try again.",
                    )
                    .await
                }
            }
        }
        Ok(None) => {
            ephemeral(
                ctx,
                cmd,
                &format!(
                    "GitHub user **{github_login}** not found. Check the username and try again."
                ),
            )
            .await
        }
        Err(e) => {
            tracing::error!(error = %e, "GitHub API error during user verification");
            ephemeral(
                ctx,
                cmd,
                "Could not verify GitHub username. Try again later.",
            )
            .await
        }
    }
}

/// Remove the invoking Discord user's GitHub link.
///
/// After unlinking, the user will appear as their GitHub login in PR
/// messages rather than being @mentioned.
pub async fn handle_unlink(
    ctx: &Context,
    cmd: &CommandInteraction,
    app_state: &AppState,
) -> Result<(), serenity::Error> {
    let discord_id = cmd.user.id.get();
    match app_state.user_store.delete(discord_id).await {
        Ok(()) => {
            info!(discord_id, "user link removed");
            ephemeral(ctx, cmd, "Your Discord to GitHub link has been removed.").await
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to remove user link");
            ephemeral(
                ctx,
                cmd,
                "Something went wrong removing user link. Try again.",
            )
            .await
        }
    }
}
