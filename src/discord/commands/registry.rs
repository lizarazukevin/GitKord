//! Central registry for `Discord` slash command modules.

use crate::discord::commands::assign::AssignModule;
use crate::discord::commands::health::HealthModule;
use crate::discord::commands::link::UserLinkModule;
use crate::discord::commands::subscribe::SubscribeModule;
use crate::service::discord::assign::AssignService;
use crate::service::discord::health::HealthService;
use crate::service::discord::link::UserLinkService;
use crate::service::discord::subscribe::SubscribeService;
use async_trait::async_trait;
use serenity::all::{Command, CommandInteraction, Context, CreateCommand, Interaction};
use std::sync::Arc;
use tracing::{error, warn};

/// Module composed of independent commands grouped by shared behavior (e.g. AssignModule).
#[async_trait]
pub(crate) trait CommandModule: Send + Sync {
    /// Slash command definition(s) part of module (e.g. `/assign`, `/unassign`).
    fn commands(&self) -> Vec<CreateCommand>;

    /// Lists all command names registered (e.g. `["link", "unlink"]`).
    fn names(&self) -> &'static [&'static str];

    /// Routes module commands to appropriate handler.
    async fn execute(&self, ctx: &Context, cmd: &CommandInteraction)
        -> Result<(), serenity::Error>;
}

/// Holds every registered `CommandModule` to expose command definitions
/// at startup and routing subsequent interactions to the module that owns it.
pub(crate) struct CommandRegistry {
    modules: Vec<Arc<dyn CommandModule>>,
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandRegistry {
    fn new() -> Self {
        Self {
            modules: Vec::new(),
        }
    }

    fn register<M: CommandModule + 'static>(&mut self, module: M) {
        self.modules.push(Arc::new(module));
    }

    /// Every registered module's command definitions, flattened into one list.
    fn all_commands(&self) -> Vec<CreateCommand> {
        self.modules.iter().flat_map(|m| m.commands()).collect()
    }

    /// Sends every module's command definitions to `Discord` as global commands.
    pub(crate) async fn register_all(&self, ctx: &Context) -> Result<(), serenity::Error> {
        Command::set_global_commands(ctx, self.all_commands()).await?;
        Ok(())
    }

    /// Routes an incoming interaction to the owning module's command `execute` method.
    pub(crate) async fn dispatch(&self, ctx: &Context, interaction: &Interaction) {
        let Interaction::Command(cmd) = interaction else {
            return;
        };
        let name = cmd.data.name.as_str();

        for module in &self.modules {
            if module.names().contains(&name) {
                if let Err(e) = module.execute(ctx, cmd).await {
                    error!(command = %name, error = %e, "command module failed");
                }
                return;
            }
        }
        warn!(command = %name, "unhandled slash command");
    }
}

/// Constructs the registry with every known module and their commands.
pub(crate) fn build_registry(
    assign_service: Arc<AssignService>,
    subscribe_service: Arc<SubscribeService>,
    link_service: Arc<UserLinkService>,
    health_service: Arc<HealthService>,
) -> CommandRegistry {
    let mut registry = CommandRegistry::new();

    registry.register(AssignModule::new(assign_service));
    registry.register(SubscribeModule::new(subscribe_service));
    registry.register(UserLinkModule::new(link_service));
    registry.register(HealthModule::new(health_service));

    registry
}
