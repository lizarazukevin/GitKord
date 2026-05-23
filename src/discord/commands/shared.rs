//! Shared utilities for slash command handlers.
//!
//! These helpers are thin wrappers around Serenity primitives used
//! consistently across every command handler.

use serenity::all::{
    CommandInteraction, Context, CreateInteractionResponse, CreateInteractionResponseMessage,
};

/// Send an ephemeral response visible only to the user who ran the command.
///
/// Used for all command responses to keep channels clean.
pub async fn ephemeral(
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

/// Extract a string option from a command interaction by name.
///
/// Returns an empty string if the option is absent or not a string type.
pub fn string_option(cmd: &CommandInteraction, name: &str) -> String {
    cmd.data
        .options
        .iter()
        .find(|o| o.name == name)
        .and_then(|o| o.value.as_str())
        .unwrap_or("")
        .to_owned()
}

/// Extract an integer option from a command interaction by name.
///
/// Returns `None` if the option is absent or not an integer type.
pub fn number_option(cmd: &CommandInteraction, name: &str) -> Option<u64> {
    cmd.data
        .options
        .iter()
        .find(|o| o.name == name)
        .and_then(|o| o.value.as_i64())
        .map(i64::cast_unsigned)
}
