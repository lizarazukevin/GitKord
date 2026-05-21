//! Slash command registration and handlers.
//!
//! All responses are ephemeral to keep channel clean.
//! Commands that need repo/PR context will infer from thread,
//! if not possible through context, otherwise explicitly.

use crate::discord::context::AppState;
use crate::discord::messages;
use crate::github::api;
use crate::state::traits::{Subscription, UserLink};
use serenity::all::{
    Command, CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption,
    CreateInteractionResponse, CreateInteractionResponseMessage, Interaction,
};
use tracing::info;

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

/// Route an incoming interaction to the proper handler.
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

async fn handle_subscribe(
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
    let payload_url = format!("{}/github/webhook", app_state.webhook_url);

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
            Check that your token has `admin:repo_hook` scope (classic) or Webhooks read/write\
            (fine-grained).",
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
            info!(channel = %cmd.channel_id, guild_id = %guild_id, repo = %repo,"channel subscribed");
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

async fn handle_unsubscribe(
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

    match app_state.sub_store.delete(&repo, guild_id.get()).await {
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

async fn handle_link(
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

async fn handle_unlink(
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

async fn handle_health(ctx: &Context, cmd: &CommandInteraction) -> Result<(), serenity::Error> {
    ephemeral(ctx, cmd, "`GitKord` is online and healthy.").await
}

enum ReviewerAction {
    Assign,
    Unassign,
}

/// Get `repo` and `pr_number` from the thread ID, defaults to explicit
/// options to look up thread ID and PR message store.
async fn resolve_pr_context(
    cmd: &CommandInteraction,
    ctx: &Context,
    app_state: &AppState,
) -> Result<Option<(String, u64)>, serenity::Error> {
    match app_state
        .pr_store
        .get_by_thread_id(cmd.channel_id.get())
        .await
    {
        Ok(Some(record)) => return Ok(Some((record.repo, record.pr_number))),
        Ok(None) => {}
        Err(e) => {
            tracing::error!(error = %e, "thread lookup failed");
            return ephemeral(ctx, cmd, "Something went wrong. Try Again.")
                .await
                .map(|()| None);
        }
    }

    let repo_opt = string_option(cmd, "repo");
    let pr_opt = number_option(cmd, "pr");

    if let (true, Some(pr_number)) = (!repo_opt.is_empty(), pr_opt) {
        Ok(Some((repo_opt, pr_number)))
    } else {
        ephemeral(
            ctx,
            cmd,
            "Run this inside a PR thread, or provide both the `repo` and `pr`.",
        )
        .await?;
        Ok(None)
    }
}

/// Resolve a reviewer input to a GitHub login.
/// Accepts either a raw username or Discord mention (<@id>).
async fn resolve_reviewer(
    ctx: &Context,
    cmd: &CommandInteraction,
    app_state: &AppState,
    reviewer: &str,
) -> Result<Option<String>, serenity::Error> {
    if !reviewer.starts_with("<@") {
        return Ok(Some(reviewer.to_owned()));
    }

    let discord_id = reviewer
        .trim_start_matches("<@")
        .trim_end_matches('>')
        .parse::<u64>()
        .ok();

    if let Some(id) = discord_id {
        match app_state.user_store.get_by_discord(id).await {
            Ok(Some(link)) => Ok(Some(link.github_login)),
            Ok(None) => {
                ephemeral(
                    ctx,
                    cmd,
                    "That Discord user has not linked their GitHub account.\
            Ask them to run `/link` first.",
                )
                .await?;
                Ok(None)
            }
            Err(e) => {
                tracing::error!(error = %e, "user store lookup failed");
                ephemeral(ctx, cmd, "Something went wrong. Try Again.").await?;
                Ok(None)
            }
        }
    } else {
        ephemeral(ctx, cmd, "Could not parse that Discord mention.").await?;
        Ok(None)
    }
}

struct ReviewerRequest {
    owner: String,
    repo_name: String,
    repo: String,
    pr_number: u64,
    github_login: String,
}

async fn validate_reviewer_request(
    ctx: &Context,
    cmd: &CommandInteraction,
    app_state: &AppState,
) -> Result<Option<ReviewerRequest>, serenity::Error> {
    let Some(guild_id) = cmd.guild_id else {
        ephemeral(ctx, cmd, "This command only works inside a server.").await?;
        return Ok(None);
    };

    let Some((repo, pr_number)) = resolve_pr_context(cmd, ctx, app_state).await? else {
        return Ok(None);
    };

    let Some((owner, repo_name)) = repo.split_once('/') else {
        ephemeral(ctx, cmd, "Provide repo in `owner/name` format.").await?;
        return Ok(None);
    };

    match app_state.sub_store.get(&repo, guild_id.get()).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            ephemeral(
                ctx,
                cmd,
                &format!("**{repo}** is not subscribed in this server. Run `/subscribe` first."),
            )
            .await?;
            return Ok(None);
        }
        Err(e) => {
            tracing::error!(error = %e, "subscription lookup failed");
            ephemeral(ctx, cmd, "Something went wrong. Try again.").await?;
            return Ok(None);
        }
    }

    let reviewer_opt = string_option(cmd, "reviewer");
    if reviewer_opt.is_empty() {
        ephemeral(ctx, cmd, "Provide a reviewer.").await?;
        return Ok(None);
    }

    let Some(github_login) = resolve_reviewer(ctx, cmd, app_state, &reviewer_opt).await? else {
        return Ok(None);
    };

    if let Ok(Some(invoker)) = app_state.user_store.get_by_discord(cmd.user.id.get()).await {
        if invoker.github_login.to_lowercase() == github_login.to_lowercase() {
            ephemeral(ctx, cmd, "You cannot request a review from yourself.").await?;
            return Ok(None);
        }
    }

    Ok(Some(ReviewerRequest {
        owner: owner.to_owned(),
        repo_name: repo_name.to_owned(),
        repo,
        pr_number,
        github_login,
    }))
}

async fn execute_reviewer_action(
    ctx: &Context,
    cmd: &CommandInteraction,
    app_state: &AppState,
    req: ReviewerRequest,
    action: ReviewerAction,
) -> Result<(), serenity::Error> {
    let ReviewerRequest {
        owner,
        repo_name,
        repo,
        pr_number,
        github_login,
    } = req;

    match action {
        ReviewerAction::Assign => {
            match api::assign_reviewer(
                &app_state.github,
                &owner,
                &repo_name,
                pr_number,
                &github_login,
            )
            .await
            {
                Ok(()) => {
                    info!(pr_number, reviewer = %github_login, repo = %repo, "reviewer assigned");
                    post_reviewer_audit(
                        ctx,
                        app_state,
                        &repo,
                        pr_number,
                        &format!(
                            "👥 **{}** requested review from **{github_login}**",
                            cmd.user.name
                        ),
                    )
                    .await;
                    ephemeral(
                        ctx, cmd,
                        &format!("Requested review from **{github_login}** on PR #**{pr_number}** in **{repo}**"),
                    )
                        .await
                }
                Err(e) => {
                    tracing::error!(error = %e, "failed to assign reviewer");
                    ephemeral(ctx, cmd, "Could not assign the reviewer. Check the PR number and that the reviewer has access to the repo.").await
                }
            }
        }

        ReviewerAction::Unassign => {
            match api::unassign_reviewer(
                &app_state.github,
                &owner,
                &repo_name,
                pr_number,
                &github_login,
            )
            .await
            {
                Ok(()) => {
                    info!(pr_number, reviewer = %github_login, repo = %repo, "reviewer unassigned");
                    post_reviewer_audit(
                        ctx,
                        app_state,
                        &repo,
                        pr_number,
                        &format!(
                            "👤 **{}** removed review request from **{github_login}**",
                            cmd.user.name
                        ),
                    )
                    .await;
                    ephemeral(
                        ctx, cmd,
                        &format!("Removed review request from **{github_login}** on PR #{pr_number} in **{repo}**."),
                    )
                        .await
                }
                Err(e) => {
                    tracing::error!(error = %e, "failed to unassign reviewer");
                    ephemeral(
                        ctx,
                        cmd,
                        "Could not remove the reviewer. Check the PR number and reviewer.",
                    )
                    .await
                }
            }
        }
    }
}

async fn perform_reviewer_action(
    ctx: &Context,
    cmd: &CommandInteraction,
    app_state: &AppState,
    action: ReviewerAction,
) -> Result<(), serenity::Error> {
    let Some(req) = validate_reviewer_request(ctx, cmd, app_state).await? else {
        return Ok(());
    };
    execute_reviewer_action(ctx, cmd, app_state, req, action).await
}

async fn post_reviewer_audit(
    ctx: &Context,
    app_state: &AppState,
    repo: &str,
    pr_number: u64,
    content: &str,
) {
    match app_state.pr_store.get(repo, pr_number).await {
        Ok(Some(record)) => {
            if let Err(e) = messages::post_to_thread(&ctx.http, record.thread_id, content).await {
                tracing::error!(error = %e, "Failed to post reviewer audit entry to thread");
            }
        }
        Ok(None) => {
            tracing::warn!(repo, pr_number, "no PR message record found for audit post");
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to look up PR record for audit post");
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Send an ephemeral response — only visible to the user who ran the command.
async fn ephemeral(
    ctx: &Context,
    cmd: &CommandInteraction,
    content: &str,
) -> Result<(), serenity::Error> {
    cmd.create_response(
        ctx,
        CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content(content)
                .ephemeral(true),
        ),
    )
    .await
}

/// Pull a string option value by name. Returns empty string if not present.
fn string_option(cmd: &CommandInteraction, name: &str) -> String {
    cmd.data
        .options
        .iter()
        .find(|o| o.name == name)
        .and_then(|o| o.value.as_str())
        .unwrap_or("")
        .to_owned()
}

/// Pull an integer option value by name. Returns empty if number is not present.
fn number_option(cmd: &CommandInteraction, name: &str) -> Option<u64> {
    cmd.data
        .options
        .iter()
        .find(|o| o.name == name)
        .and_then(|o| o.value.as_i64())
        .map(i64::cast_unsigned)
}
