//! Slash command definitions and handlers for `DiGiBot`.
//!
//! All responses are ephemeral — only the invoking user sees them,
//! keeping the channel free of bot noise.

use crate::github::api;
use crate::state::traits::{Subscription, SubscriptionStore, UserLink, UserLinkStore};
use crate::state::PrMessageStore;
use octocrab::Octocrab;
use serenity::all::{
    ChannelId, Command, CommandInteraction, CommandOptionType, Context, CreateCommand,
    CreateCommandOption, CreateInteractionResponse, CreateInteractionResponseMessage,
    CreateMessage, Interaction,
};
use tracing::info;
// ── Registration ──────────────────────────────────────────────────────────────

/// Register all global slash commands with Discord.
///
/// Called once in the `ready` event. Global commands propagate to all guilds
/// within an hour; for faster iteration during development use guild commands.
///
/// # Errors
///
/// Returns a [`serenity::Error`] if the API call fails.
pub async fn register(ctx: &Context) -> Result<(), serenity::Error> {
    Command::set_global_commands(
        ctx,
        vec![
            CreateCommand::new("subscribe")
                .description("Subscribe this channel to PR updates for a repository")
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "repo",
                        "Repository to watch in owner/name format (e.g. kevinlizarazu/digibot)",
                    )
                    .required(true),
                ),
            CreateCommand::new("unsubscribe")
                .description("Stop posting PR updates for a repository")
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "repo",
                        "Repository to unsubscribe from in owner/name format",
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
            CreateCommand::new("unlink").description("Remove your Discord ↔ GitHub link"),
            CreateCommand::new("health").description("Show DiGiBot status"),
            CreateCommand::new("assign")
                .description("Request a review on a pull request")
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "reviewer",
                        "GitHub username or @Discord mention of the reviewer",
                    )
                    .required(true),
                )
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "repo",
                        "Repository in owner/name format (must be subscribed)",
                    )
                    .required(false),
                )
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::Integer,
                        "pr",
                        "Pull request number",
                    )
                    .required(false),
                ),
            CreateCommand::new("unassign")
                .description("Remove a review request")
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "reviewer",
                        "Github username or @Discord mention of the reviewer",
                    )
                    .required(true),
                )
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "repo",
                        "Repository in owner/name format",
                    )
                    .required(false),
                )
                .add_option(
                    CreateCommandOption::new(CommandOptionType::Integer, "pr", "PR number")
                        .required(false),
                ),
        ],
    )
    .await?;

    info!("slash commands registered");
    Ok(())
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

/// Route an incoming interaction to the correct command handler.
#[allow(clippy::too_many_arguments)]
pub async fn dispatch(
    ctx: &Context,
    interaction: &Interaction,
    pr_store: &dyn PrMessageStore,
    sub_store: &dyn SubscriptionStore,
    user_store: &dyn UserLinkStore,
    github: &Octocrab,
    webhook_url: &str,
    webhook_secret: &str,
) {
    let Interaction::Command(cmd) = interaction else {
        return;
    };

    let result = match cmd.data.name.as_str() {
        "subscribe" => {
            handle_subscribe(ctx, cmd, sub_store, github, webhook_url, webhook_secret).await
        }
        "unsubscribe" => handle_unsubscribe(ctx, cmd, sub_store).await,
        "link" => handle_link(ctx, cmd, user_store, github).await,
        "unlink" => handle_unlink(ctx, cmd, user_store).await,
        "health" => handle_health(ctx, cmd).await,
        "assign" => handle_assign(ctx, cmd, pr_store, sub_store, user_store, github).await,
        "unassign" => handle_unassign(ctx, cmd, pr_store, sub_store, user_store, github).await,
        other => {
            tracing::warn!(command = %other, "unhandled slash command");
            return;
        }
    };

    if let Err(e) = result {
        tracing::error!(error = %e, "slash command handler failed");
    }
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn handle_subscribe(
    ctx: &Context,
    cmd: &CommandInteraction,
    sub_store: &dyn SubscriptionStore,
    github: &Octocrab,
    webhook_url: &str,
    webhook_secret: &str,
) -> Result<(), serenity::Error> {
    // guild_id only made available in servers, not in DMs
    let Some(guild_id) = cmd.guild_id else {
        return ephemeral(ctx, cmd, "❌ This command can only be used in a server").await;
    };

    let repo = cmd
        .data
        .options
        .iter()
        .find(|o| o.name == "repo")
        .and_then(|o| o.value.as_str())
        .unwrap_or("")
        .to_owned();

    if repo.is_empty() || !repo.contains('/') {
        return ephemeral(
            ctx,
            cmd,
            "❌ Please provide a valid repo in `owner/name` format.",
        )
        .await;
    }

    let (owner, repo_name) = repo.split_once('/').expect("validated above");

    let payload_url = format!("{webhook_url}/github/webhook");
    match api::register_webhook(github, owner, repo_name, &payload_url, webhook_secret).await {
        Ok(Some(id)) => info!(repo, hook_id = id, "webhook registered via API"),
        Ok(None) => info!(repo, "webhook already existed"),
        Err(e) => {
            tracing::error!(error = %e, repo, "failed to register webhook");
            return ephemeral(ctx, cmd, "❌ Could not register webhook on GitHub. Check that your token has `admin:repo_hook` scope.").await;
        }
    }

    let subscription = Subscription {
        repo: repo.clone(),
        guild_id: guild_id.get(),
        channel_id: cmd.channel_id.get(),
    };

    match sub_store.upsert(subscription).await {
        Ok(()) => {
            info!(
                channel = %cmd.channel_id,
                guild_id = %guild_id,
                repo = %repo,
                "channel subscribed"
            );
            ephemeral(
                ctx,
                cmd,
                &format!("✅ This channel will now receive PR updates for **{repo}**."),
            )
            .await
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to save subscription");
            ephemeral(ctx, cmd, "❌ Something went wrong. Try again.").await
        }
    }
}

async fn handle_unsubscribe(
    ctx: &Context,
    cmd: &CommandInteraction,
    sub_store: &dyn SubscriptionStore,
) -> Result<(), serenity::Error> {
    let Some(guild_id) = cmd.guild_id else {
        return ephemeral(ctx, cmd, "❌ This command can only be used in a server.").await;
    };

    let repo = cmd
        .data
        .options
        .iter()
        .find(|o| o.name == "repo")
        .and_then(|o| o.value.as_str())
        .unwrap_or("")
        .to_owned();

    if repo.is_empty() {
        return ephemeral(ctx, cmd, "❌ Please provide a repo in `owner/name` format.").await;
    }

    match sub_store.delete(&repo, guild_id.get()).await {
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
                &format!("🔕 This channel will no longer receive PR updates for **{repo}**."),
            )
            .await
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to remove subscription");
            ephemeral(ctx, cmd, "❌ Something went wrong. Try again.").await
        }
    }
}

async fn handle_link(
    ctx: &Context,
    cmd: &CommandInteraction,
    user_store: &dyn UserLinkStore,
    github: &Octocrab,
) -> Result<(), serenity::Error> {
    let github_login = cmd
        .data
        .options
        .iter()
        .find(|o| o.name == "username")
        .and_then(|o| o.value.as_str())
        .unwrap_or("")
        .to_owned();

    if github_login.is_empty() {
        return ephemeral(ctx, cmd, "❌ Please provide your GitHub username.").await;
    }

    match api::verify_user(github, &github_login).await {
        Ok(Some(verified_login)) => {
            let discord_id = cmd.user.id.get();
            match user_store
                .upsert(UserLink {
                    discord_id,
                    github_login: verified_login.clone(),
                })
                .await
            {
                Ok(()) => {
                    info!(discord_id, github_login = %verified_login, "user link saved");
                    ephemeral(
                        ctx,
                        cmd,
                        &format!(
                            "✅ Linked your Discord account to **{verified_login}** on GitHub."
                        ),
                    )
                    .await
                }
                Err(e) => {
                    tracing::error!(error = %e, "failed to save user link");
                    ephemeral(
                        ctx,
                        cmd,
                        "❌ Something went wrong saving your link. Try again.",
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
                "❌ GitHub user **{github_login}** not found. Check the username and try again."
            ),
            )
            .await
        }
        Err(e) => {
            tracing::error!(error = %e, "GitHub API error during user verification");
            ephemeral(
                ctx,
                cmd,
                "❌ Could not verify GitHub username. Try again later.",
            )
            .await
        }
    }
}

async fn handle_unlink(
    ctx: &Context,
    cmd: &CommandInteraction,
    user_store: &dyn UserLinkStore,
) -> Result<(), serenity::Error> {
    let discord_id = cmd.user.id.get();

    match user_store.delete(discord_id).await {
        Ok(()) => {
            info!(discord_id, "user link removed");
            ephemeral(ctx, cmd, "✅ Your Discord ↔ GitHub link has been removed.").await
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to remove user link");
            ephemeral(ctx, cmd, "❌ Something went wrong. Try again.").await
        }
    }
}

async fn handle_health(ctx: &Context, cmd: &CommandInteraction) -> Result<(), serenity::Error> {
    ephemeral(ctx, cmd, "🟢 `DiGiBot` is online and healthy.").await
}

enum ReviewerAction {
    Assign,
    Unassign,
}

async fn resolve_pr_context(
    cmd: &CommandInteraction,
    pr_store: &dyn PrMessageStore,
    ctx: &Context,
) -> Result<Option<(String, u64)>, serenity::Error> {
    let repo_opt = cmd
        .data
        .options
        .iter()
        .find(|o| o.name == "repo")
        .and_then(|o| o.value.as_str())
        .map(str::to_owned);

    let pr_opt = cmd
        .data
        .options
        .iter()
        .find(|o| o.name == "pr")
        .and_then(|o| o.value.as_i64())
        .map(i64::cast_unsigned);

    match (repo_opt, pr_opt) {
        (Some(repo), Some(pr_number)) => Ok(Some((repo, pr_number))),

        _ => match pr_store.get_by_thread_id(cmd.channel_id.get()).await {
            Ok(Some(record)) => Ok(Some((record.repo, record.pr_number))),
            Ok(None) => {
                ephemeral(
                        ctx,
                        cmd,
                        "❌ Run this command inside a PR audit thread, or provide both `repo` and `pr`.",
                    )
                        .await?;
                Ok(None)
            }
            Err(e) => {
                tracing::error!(error = %e, "thread lookup failed");
                ephemeral(ctx, cmd, "❌ Something went wrong. Try again.").await?;
                Ok(None)
            }
        },
    }
}

#[allow(clippy::too_many_lines)]
async fn perform_reviewer_action(
    ctx: &Context,
    cmd: &CommandInteraction,
    pr_store: &dyn PrMessageStore,
    sub_store: &dyn SubscriptionStore,
    user_store: &dyn UserLinkStore,
    github: &Octocrab,
    action: ReviewerAction,
) -> Result<(), serenity::Error> {
    let Some(guild_id) = cmd.guild_id else {
        return ephemeral(ctx, cmd, "❌ This command can only be used in a server.").await;
    };

    // Resolve repo + PR number.
    let Some((repo, pr_number)) = resolve_pr_context(cmd, pr_store, ctx).await? else {
        return Ok(());
    };

    if !repo.contains('/') {
        return ephemeral(ctx, cmd, "❌ Please provide repo in `owner/name` format.").await;
    }

    match sub_store.get(&repo, guild_id.get()).await {
        Ok(None) => {
            return ephemeral(
                ctx,
                cmd,
                &format!("❌ **{repo}** is not subscribed in this server. Run `/subscribe` first."),
            )
            .await;
        }
        Err(e) => {
            tracing::error!(error = %e, "subscription lookup failed");
            return ephemeral(ctx, cmd, "❌ Something went wrong. Try again.").await;
        }
        Ok(Some(_)) => {}
    }

    let reviewer_input = cmd
        .data
        .options
        .iter()
        .find(|o| o.name == "reviewer")
        .and_then(|o| o.value.as_str())
        .unwrap_or("")
        .to_owned();

    if reviewer_input.is_empty() {
        return ephemeral(ctx, cmd, "❌ Please provide a reviewer.").await;
    }

    let github_login = if reviewer_input.starts_with("<@") {
        let discord_id = reviewer_input
            .trim_start_matches("<@")
            .trim_end_matches('>')
            .parse::<u64>()
            .ok();

        match discord_id {
            Some(id) => match user_store.get_by_discord(id).await {
                Ok(Some(link)) => link.github_login,
                Ok(None) => {
                    return ephemeral(
                        ctx,
                        cmd,
                        "❌ That Discord user has not linked their GitHub account. Ask them to run `/link`.",
                    ).await;
                }
                Err(e) => {
                    tracing::error!(error = %e, "user store lookup failed");
                    return ephemeral(ctx, cmd, "❌ Something went wrong. Try again.").await;
                }
            },
            None => return ephemeral(ctx, cmd, "❌ Could not parse that Discord mention.").await,
        }
    } else {
        reviewer_input.clone()
    };

    // Self-assignment guard.
    if let Ok(Some(invoker)) = user_store.get_by_discord(cmd.user.id.get()).await {
        if invoker.github_login.to_lowercase() == github_login.to_lowercase() {
            return ephemeral(ctx, cmd, "❌ You cannot request a review from yourself.").await;
        }
    }

    let (owner, repo_name) = repo.split_once('/').expect("validated above");

    match action {
        ReviewerAction::Assign => {
            match api::assign_reviewer(github, owner, repo_name, pr_number, &github_login).await {
                Ok(()) => {
                    info!(pr_number, reviewer = %github_login, repo = %repo, "reviewer assigned");

                    if let Ok(Some(record)) = pr_store.get(&repo, pr_number).await {
                        let audit = format!(
                            "👥 **{}** requested review from **{github_login}**",
                            cmd.user.name
                        );
                        ChannelId::new(record.thread_id)
                            .send_message(&ctx.http, CreateMessage::new().content(audit))
                            .await
                            .ok();
                    }

                    ephemeral(
                        ctx,
                        cmd,
                        &format!("👥 Requested review from **{github_login}** on PR #{pr_number} in **{repo}**."),
                    ).await
                }
                Err(e) => {
                    tracing::error!(error = %e, "failed to assign reviewer");
                    ephemeral(ctx, cmd, "❌ Could not assign reviewer. Check the PR number and that the reviewer has access to the repo.").await
                }
            }
        }
        ReviewerAction::Unassign => {
            match api::unassign_reviewer(github, owner, repo_name, pr_number, &github_login).await {
                Ok(()) => {
                    info!(pr_number, reviewer = %github_login, repo = %repo, "reviewer unassigned");

                    if let Ok(Some(record)) = pr_store.get(&repo, pr_number).await {
                        let audit = format!(
                            "👤 **{}** removed review request from **{github_login}**",
                            cmd.user.name
                        );
                        ChannelId::new(record.thread_id)
                            .send_message(&ctx.http, CreateMessage::new().content(audit))
                            .await
                            .ok();
                    }

                    ephemeral(
                        ctx,
                        cmd,
                        &format!("👤 Removed review request from **{github_login}** on PR #{pr_number} in **{repo}**."),
                    ).await
                }
                Err(e) => {
                    tracing::error!(error = %e, "failed to unassign reviewer");
                    ephemeral(
                        ctx,
                        cmd,
                        "❌ Could not remove reviewer. Check the PR number and reviewer.",
                    )
                    .await
                }
            }
        }
    }
}

async fn handle_assign(
    ctx: &Context,
    cmd: &CommandInteraction,
    pr_store: &dyn PrMessageStore,
    sub_store: &dyn SubscriptionStore,
    user_store: &dyn UserLinkStore,
    github: &Octocrab,
) -> Result<(), serenity::Error> {
    perform_reviewer_action(
        ctx,
        cmd,
        pr_store,
        sub_store,
        user_store,
        github,
        ReviewerAction::Assign,
    )
    .await
}

async fn handle_unassign(
    ctx: &Context,
    cmd: &CommandInteraction,
    pr_store: &dyn PrMessageStore,
    sub_store: &dyn SubscriptionStore,
    user_store: &dyn UserLinkStore,
    github: &Octocrab,
) -> Result<(), serenity::Error> {
    perform_reviewer_action(
        ctx,
        cmd,
        pr_store,
        sub_store,
        user_store,
        github,
        ReviewerAction::Unassign,
    )
    .await
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
