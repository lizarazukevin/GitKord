//! Slash command definitions and handlers for `DiGiBot`.
//!
//! All responses are ephemeral — only the invoking user sees them,
//! keeping the channel free of bot noise.

use crate::github::api;
use crate::state::traits::{Subscription, SubscriptionStore, UserLink, UserLinkStore};
use octocrab::Octocrab;
use serenity::all::{
    Command, CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption,
    CreateInteractionResponse, CreateInteractionResponseMessage, Interaction,
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
        ],
    )
    .await?;

    info!("slash commands registered");
    Ok(())
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

/// Route an incoming interaction to the correct command handler.
pub async fn dispatch(
    ctx: &Context,
    interaction: &Interaction,
    sub_store: &dyn SubscriptionStore,
    user_store: &dyn UserLinkStore,
    github: &Octocrab,
    webhook_url: &str,
) {
    let Interaction::Command(cmd) = interaction else {
        return;
    };

    let result = match cmd.data.name.as_str() {
        "subscribe" => handle_subscribe(ctx, cmd, sub_store, github, webhook_url).await,
        "unsubscribe" => handle_unsubscribe(ctx, cmd, sub_store).await,
        "link" => handle_link(ctx, cmd, user_store, github).await,
        "unlink" => handle_unlink(ctx, cmd, user_store).await,
        "health" => handle_health(ctx, cmd).await,
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

    // refactor later to pass through WebhookState
    let secret = std::env::var("GITHUB_WEBHOOK_SECRET").unwrap_or_default();

    match api::register_webhook(github, owner, repo_name, &payload_url, &secret).await {
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
