//! Slash command registration and dispatch.
//!
//! This module is the entry point for all Discord slash commands.
//! `register` publishes the command definitions to Discord on startup,
//! and `dispatch` routes incoming interactions to the correct handler.
//!
//! Each command group lives in its own submodule to keep this file
//! focused on routing only.

use crate::discord::commands::health::handle_health;
use crate::discord::commands::reviewer::perform_reviewer_action;
use crate::discord::commands::subscription::{handle_subscribe, handle_unsubscribe};
use crate::discord::commands::user_link::{handle_link, handle_unlink};
use crate::discord::context::AppState;
use crate::discord::models::ReviewerAction;
use serenity::all::{
    Command, CommandOptionType, Context, CreateCommand, CreateCommandOption, Interaction,
};
use tracing::info;

pub mod health;
pub mod reviewer;
pub mod shared;
pub mod subscription;
pub mod user_link;

/// Register all global slash commands with Discord.
///
/// Global commands take up to an hour to propagate. During development,
/// guild-scoped commands register instantly for faster iteration.
///
/// # Errors
///
/// Returns a [`serenity::Error`] if the API call fails.
pub async fn register(ctx: &Context) -> Result<(), serenity::Error> {
    Command::set_global_commands(
        ctx,
        vec![
            CreateCommand::new("subscribe")
                .description("Subscribe this channel to receive PR updates for a repository")
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "repo",
                        "Repository to watch in owner/name format (e.g. kevinlizarazu/gitkord)",
                    )
                    .required(true),
                ),
            CreateCommand::new("unsubscribe")
                .description("Stop posting PR updates for a repository in this channel")
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "repo",
                        "Repository in owner/name format",
                    )
                    .required(true),
                ),
            CreateCommand::new("link")
                .description("Link your Discord account to a GitHub username")
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "username",
                        "Your GitHub username",
                    )
                    .required(true),
                ),
            CreateCommand::new("unlink").description("Remove your Discord to GitHub account link"),
            CreateCommand::new("health").description("Check if GitKord is running"),
            CreateCommand::new("assign")
                .description(
                    "Request a review on a PR. Run inside a PR thread to skip `repo` and `pr`.",
                )
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "reviewer",
                        "GitHub username or @Discord mention",
                    )
                    .required(true),
                )
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "repo",
                        "Repository in owner/name format (not needed inside a PR thread)",
                    )
                    .required(false),
                )
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::Integer,
                        "pr",
                        "PR number (not needed inside a PR thread)",
                    )
                    .required(false),
                ),
            CreateCommand::new("unassign")
                .description("Remove a review request. Run inside a PR thread to skip repo and pr.")
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "reviewer",
                        "Github username or @Discord mention",
                    )
                    .required(true),
                )
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "repo",
                        "Repository in owner/name format (not needed inside a PR thread)",
                    )
                    .required(false),
                )
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::Integer,
                        "pr",
                        "PR number (not needed inside a PR thread)",
                    )
                    .required(false),
                ),
        ],
    )
    .await?;

    info!("slash commands registered");
    Ok(())
}

/// Route an incoming interaction to the correct handler.
///
/// Non-command interactions are silently ignored. Handler errors are
/// logged but not propagated. Failed command should be handled gracefully.
pub async fn dispatch(ctx: &Context, interaction: &Interaction, app_state: &AppState) {
    let Interaction::Command(cmd) = interaction else {
        return;
    };

    let result = match cmd.data.name.as_str() {
        "subscribe" => handle_subscribe(ctx, cmd, app_state).await,
        "unsubscribe" => handle_unsubscribe(ctx, cmd, app_state).await,
        "link" => handle_link(ctx, cmd, app_state).await,
        "unlink" => handle_unlink(ctx, cmd, app_state).await,
        "health" => handle_health(ctx, cmd).await,
        "assign" => perform_reviewer_action(ctx, cmd, app_state, ReviewerAction::Assign).await,
        "unassign" => perform_reviewer_action(ctx, cmd, app_state, ReviewerAction::Unassign).await,
        other => {
            tracing::warn!(command = %other, "unhandled slash command");
            return;
        }
    };

    if let Err(e) = result {
        tracing::error!(error = %e, "slash command handler failed");
    }
}
