//! Health check command.
//!
//! Reports the status of both the Discord bot and the GitHub App integration.
//! Performs a lightweight API call to verify the GitHub App is reachable.

use crate::constants::APP_NAME;
use crate::discord::commands::shared::ephemeral;
use crate::discord::context::AppState;
use serenity::all::{CommandInteraction, Context};

/// Respond with a simple online confirmation message.
pub async fn handle_health(
    ctx: &Context,
    cmd: &CommandInteraction,
    app_state: &AppState,
) -> Result<(), serenity::Error> {
    let github_status = match app_state.github.current().app().await {
        Ok(_) => "✅ GitHub App",
        Err(_) => "❌ GitHub App",
    };

    ephemeral(
        ctx,
        cmd,
        &format!("`{APP_NAME}` is online and healthy.\n✅ Bot\n{github_status}"),
    )
    .await
}
