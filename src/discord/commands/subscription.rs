//! Subscribe and unsubscribe command handlers.
//!
//! Manages which Discord channels receive PR update messages for a given
//! repository. Subscribing also registers a GitHub webhook so events
//! start flowing immediately.

use crate::db::models::Subscription;
use crate::discord::commands::shared::{ephemeral, string_option};
use crate::discord::context::AppState;
use crate::github::api;
use serenity::all::{CommandInteraction, Context};
use tracing::info;

/// Subscribe the current channel to PR updates for a repository.
///
/// Registers a GitHub webhook on the repo if one does not already exist,
/// then stores the channel subscription so incoming events know where to post.
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

    let (owner, repo_name) = repo.split_once('/').expect("checked above");
    let payload_url = format!("https://{}/github/webhook", app_state.public_domain);

    match api::register_webhook(
        &app_state.github,
        owner,
        repo_name,
        &payload_url,
        &app_state.webhook_secret,
    )
    .await
    {
        Ok(Some(id)) => info!(repo, hook_id = id, "webhook registered"),
        Ok(None) => info!(repo, "webhook already existed"),
        Err(e) => {
            tracing::error!(error = %e, repo, "failed to register webhook");
            return ephemeral(
                ctx,
                cmd,
                "Could not register webhook on GitHub. \
                 Check that your token has `admin:repo_hook` scope (classic) \
                 or Webhooks read/write (fine-grained).",
            )
            .await;
        }
    }

    match app_state
        .sub_store
        .upsert(Subscription {
            repo: repo.clone(),
            guild_id: guild_id.get(),
            channel_id: cmd.channel_id.get(),
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
