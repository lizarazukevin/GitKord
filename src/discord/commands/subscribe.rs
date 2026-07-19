//! Subscribe and unsubscribe command handlers.

use crate::discord::commands::args::{repo_name_option, CommandOption};
use crate::discord::commands::registry::CommandModule;
use crate::discord::commands::response::{ephemeral, reject_if_thread, require_guild};
use crate::error::format_error;
use crate::service::discord::subscribe::{SubscribeAction, SubscribeRequest, SubscribeService};
use crate::AppError;
use anyhow::Result;
use async_trait::async_trait;
use serenity::all::{
    CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption,
};
use serenity::Error;
use std::sync::Arc;
use tracing::error;

pub(super) struct SubscribeModule {
    service: Arc<SubscribeService>,
}

impl SubscribeModule {
    pub(super) fn new(service: Arc<SubscribeService>) -> Self {
        Self { service }
    }

    async fn parse_subscribe_input(
        &self,
        ctx: &Context,
        cmd: &CommandInteraction,
    ) -> Result<Option<SubscribeRequest>, Error> {
        let Some(guild_id) = require_guild(ctx, cmd).await? else {
            return Ok(None);
        };

        let action = match cmd.data.name.as_str() {
            "subscribe" => SubscribeAction::Subscribe,
            "unsubscribe" => SubscribeAction::Unsubscribe,
            _ => return Ok(None),
        };

        let repo = match repo_name_option(cmd, "repository") {
            CommandOption::Valid(repo) => repo,
            _ => {
                ephemeral(
                    ctx,
                    cmd,
                    "Provide a valid repository in `owner/name` format.",
                )
                .await?;
                return Ok(None);
            }
        };

        Ok(Some(SubscribeRequest {
            repo,
            action,
            guild_id: guild_id.get(),
            channel_id: cmd.channel_id.get(),
        }))
    }
}

#[async_trait]
impl CommandModule for SubscribeModule {
    fn commands(&self) -> Vec<CreateCommand> {
        vec![subscribe_command(), unsubscribe_command()]
    }

    fn names(&self) -> &'static [&'static str] {
        &["subscribe", "unsubscribe"]
    }

    async fn execute(&self, ctx: &Context, cmd: &CommandInteraction) -> Result<(), Error> {
        if reject_if_thread(
            ctx,
            cmd,
            "Subscriptions can only be managed from a main channel, not inside a thread.",
        )
        .await?
        {
            return Ok(());
        }

        let Some(req) = self.parse_subscribe_input(ctx, cmd).await? else {
            return Ok(());
        };

        match self.service.handle(req).await {
            Ok(msg) => ephemeral(ctx, cmd, &msg).await,
            Err(e) => {
                error!(error = %e, "subscribe service failed");
                let user_msg = match &e {
                    AppError::Message(_) => e.to_string(),
                    _ => format_error("Something went wrong. Please try again.", None),
                };
                ephemeral(ctx, cmd, &user_msg).await
            }
        }
    }
}

fn subscribe_command() -> CreateCommand {
    CreateCommand::new("subscribe")
        .description("Subscribe this channel to receive PR updates for a repository")
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "repository",
                "Repository to watch in owner/name format",
            )
            .required(true),
        )
}

fn unsubscribe_command() -> CreateCommand {
    CreateCommand::new("unsubscribe")
        .description("Stop posting PR updates for a repository in this channel")
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "repository",
                "Repository in owner/name format",
            )
            .required(true),
        )
}
