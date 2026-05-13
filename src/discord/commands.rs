//! Slash command definitions and handlers for `DiGiBot`.
//!
//! All responses are ephemeral — only the invoking user sees them,
//! keeping the channel free of bot noise.

use serenity::all::{
    Command, CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption,
    CreateInteractionResponse, CreateInteractionResponseMessage, Interaction,
};
use tracing::info;

use crate::state::traits::{UserLink, UserLinkStore};

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
                .description("Subscribe this channel to PR updates for the configured repository"),
            CreateCommand::new("unsubscribe")
                .description("Stop posting PR updates to this channel"),
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
pub async fn dispatch(ctx: &Context, interaction: &Interaction, user_store: &dyn UserLinkStore) {
    let Interaction::Command(cmd) = interaction else {
        return;
    };

    let result = match cmd.data.name.as_str() {
        "subscribe" => handle_subscribe(ctx, cmd).await,
        "unsubscribe" => handle_unsubscribe(ctx, cmd).await,
        "link" => handle_link(ctx, cmd, user_store).await,
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

async fn handle_subscribe(ctx: &Context, cmd: &CommandInteraction) -> Result<(), serenity::Error> {
    // Full implementation comes with the subscription store in a later step.
    // For now, acknowledges the command and informs the user.
    info!(
        channel = %cmd.channel_id,
        user    = %cmd.user.name,
        "subscribe command received"
    );

    ephemeral(
        ctx,
        cmd,
        "✅ This channel will receive PR updates. (Subscription persistence coming soon)",
    )
    .await
}

async fn handle_unsubscribe(
    ctx: &Context,
    cmd: &CommandInteraction,
) -> Result<(), serenity::Error> {
    info!(
        channel = %cmd.channel_id,
        user    = %cmd.user.name,
        "unsubscribe command received"
    );

    ephemeral(
        ctx,
        cmd,
        "🔕 PR updates will stop for this channel. (Subscription persistence coming soon)",
    )
    .await
}

async fn handle_link(
    ctx: &Context,
    cmd: &CommandInteraction,
    user_store: &dyn UserLinkStore,
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

    let discord_id = cmd.user.id.get();

    match user_store
        .upsert(UserLink {
            discord_id,
            github_login: github_login.clone(),
        })
        .await
    {
        Ok(()) => {
            info!(discord_id, github_login = %github_login, "user link saved");
            ephemeral(
                ctx,
                cmd,
                &format!("✅ Linked your Discord account to **{github_login}** on GitHub."),
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
