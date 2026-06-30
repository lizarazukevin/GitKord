//! Subscribe and unsubscribe command handlers.
//!
//! Manages which Discord channels receive PR update messages for a given
//! repository. Subscribing verifies the GitHub App is installed on the
//! repository so webhook events start flowing automatically.

use crate::constants::{APP_NAME, GITHUB_APP_URL};
use crate::db::models::Subscription;
use crate::discord::commands::shared::{ephemeral, string_option};
use crate::discord::context::AppState;
use crate::github;
use crate::github::api;
use serenity::all::{CommandInteraction, Context};
use tracing::info;

/// Subscribe the current channel to PR updates for a repository.
///
/// Verifies the GitHub App is installed on the repo, then stores the
/// channel subscription so incoming webhook events know where to post.
pub async fn handle_subscribe(
    ctx: &Context,
    cmd: &CommandInteraction,
    app_state: &AppState,
) -> Result<(), serenity::Error> {
    let Some(guild_id) = cmd.guild_id else {
        return ephemeral(ctx, cmd, "This command only works inside a server.").await;
    };

    let repo = string_option(cmd, "repo");
    if repo.is_empty() || !repo.contains('/') {
        return ephemeral(ctx, cmd, "Provide a repo in `owner/name` format.").await;
    }

    let repo = repo.to_lowercase();
    let (owner, repo_name) = repo.split_once('/').expect("checked above");

    let installation_id = if app_state.local_dev {
        // In local dev: register a webhook on the repo pointing at the ngrok URL,
        // then store a dummy installation ID since the GitHub App isn't installed.
        let payload_url = format!("https://{}/github/webhook", app_state.public_domain);
        match api::register_webhook(
            app_state.github.inner(),
            owner,
            repo_name,
            &payload_url,
            &app_state.webhook_secret,
        )
        .await
        {
            Ok(_) => {
                info!(repo, "webhook registered for local dev");
            }
            Err(e) => {
                tracing::error!(error = %e, repo, "failed to register webhook");
                return ephemeral(
                    ctx,
                    cmd,
                    &format!(
                        "Could not register webhook for **{repo}**. \
                         Make sure your PAT has admin access to the repo.",
                    ),
                )
                .await;
            }
        }
        0 // dummy installation ID
    } else {
        // Production: verify the app is installed on this repo
        match github::client::get_installation_id(app_state.github.inner(), owner, repo_name).await
        {
            Ok(id) => id,
            Err(e) => {
                tracing::error!(error = %e, repo, "failed to get installation ID");
                return ephemeral(
                    ctx,
                    cmd,
                    &format!(
                        "{APP_NAME} is not installed on that repository. \
         Install it at {GITHUB_APP_URL} first, \
         then run `/subscribe` again."
                    ),
                )
                .await;
            }
        }
    };

    match app_state
        .sub_store
        .upsert(Subscription {
            repo: repo.clone(),
            guild_id: guild_id.get(),
            channel_id: cmd.channel_id.get(),
            installation_id,
        })
        .await
    {
        Ok(()) => {
            info!(
                channel  = %cmd.channel_id,
                guild_id = %guild_id,
                repo     = %repo,
                "channel subscribed"
            );
            ephemeral(
                ctx,
                cmd,
                &format!("This channel will now receive PR updates for **{repo}**."),
            )
            .await
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to save subscription");
            ephemeral(
                ctx,
                cmd,
                "Something went wrong saving the subscription. Try again.",
            )
            .await
        }
    }
}

/// Unsubscribe the current channel from PR updates for a repository.
///
/// Removes the stored subscription so future webhook events are no longer
/// posted here. Does not remove the GitHub webhook since other channels
/// in the same server may still be subscribed.
pub async fn handle_unsubscribe(
    ctx: &Context,
    cmd: &CommandInteraction,
    app_state: &AppState,
) -> Result<(), serenity::Error> {
    let Some(guild_id) = cmd.guild_id else {
        return ephemeral(ctx, cmd, "This command can only be used in a server.").await;
    };

    let repo = string_option(cmd, "repo");
    if repo.is_empty() {
        return ephemeral(ctx, cmd, "Provide a repo in `owner/name` format.").await;
    }

    let repo = repo.to_lowercase();
    match app_state
        .sub_store
        .delete(&repo, guild_id.get(), cmd.channel_id.get())
        .await
    {
        Ok(()) => {
            info!(
                channel = %cmd.channel_id,
                guild   = %guild_id,
                repo    = %repo,
                "channel unsubscribed"
            );
            ephemeral(
                ctx,
                cmd,
                &format!("This channel will no longer receive PR updates for **{repo}**."),
            )
            .await
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to remove subscription");
            ephemeral(
                ctx,
                cmd,
                "Something went wrong removing subscription. Try again.",
            )
            .await
        }
    }
}
