//! Assign and unassign command handlers.
//!
//! Manages reviewer requests on GitHub PRs from within Discord.
//! Commands can be run inside a PR audit thread to infer context,
//! or with explicit `repo` and `pr` options from any channel.

use crate::constants::{APP_NAME, GITHUB_APP_URL};
use crate::discord::commands::shared::{ephemeral, number_option, string_option};
use crate::discord::context::AppState;
use crate::discord::messages;
use crate::discord::models::{ReviewerAction, ReviewerRequest};
use crate::github::api;
use octocrab::Octocrab;
use serenity::all::{CommandInteraction, Context};
use tracing::info;

/// Entry point for `/assign` and `/unassign`.
///
/// Validates all preconditions before attempting the GitHub API call.
/// Returns early with an ephemeral error if any validation step fails.
pub async fn perform_reviewer_action(
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

/// Validate all preconditions for a reviewer action.
///
/// Checks in order: guild presence, PR context, repo format, subscription,
/// reviewer option, reviewer resolution, and self-review guard.
/// Returns `None` if any check fails, the handler has already sent an
/// ephemeral reply so the caller just returns `Ok(())`.
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

    match app_state
        .sub_store
        .get_by_guild(&repo, guild_id.get())
        .await
    {
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

/// Execute the GitHub API call and post the audit entry.
///
/// Only called after all validation has passed. Handles both assign
/// and unassign in one place to keep the action symmetry obvious.
async fn execute_reviewer_action(
    ctx: &Context,
    cmd: &CommandInteraction,
    app_state: &AppState,
    req: ReviewerRequest,
    action: ReviewerAction,
) -> Result<(), serenity::Error> {
    let Some(installation) = get_installation(app_state, &req.repo, ctx, cmd).await? else {
        return Ok(());
    };

    match action {
        ReviewerAction::Assign => assign(ctx, cmd, app_state, &installation, req).await,
        ReviewerAction::Unassign => unassign(ctx, cmd, app_state, &installation, req).await,
    }
}

async fn assign(
    ctx: &Context,
    cmd: &CommandInteraction,
    app_state: &AppState,
    installation: &Octocrab,
    req: ReviewerRequest,
) -> Result<(), serenity::Error> {
    match api::assign_reviewer(
        installation,
        &req.owner,
        &req.repo_name,
        req.pr_number,
        &req.github_login,
    )
    .await
    {
        Ok(()) => {
            info!(req.pr_number, reviewer = %req.github_login, repo = %req.repo, "reviewer assigned");
            post_reviewer_audit(
                ctx,
                app_state,
                &req.repo,
                req.pr_number,
                &cmd.user.name,
                &req.github_login,
                true,
            )
            .await;
            ephemeral(
                ctx,
                cmd,
                &format!(
                    "Requested review from **{}** on PR #**{}** in **{}**",
                    req.github_login, req.pr_number, req.repo,
                ),
            )
            .await
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to assign reviewer");
            ephemeral(ctx, cmd, &format!("Could not assign the reviewer. Check the PR number and that the reviewer has access to the repo. \
            Also make sure {APP_NAME} is installed: {GITHUB_APP_URL}")).await
        }
    }
}

async fn unassign(
    ctx: &Context,
    cmd: &CommandInteraction,
    app_state: &AppState,
    installation: &Octocrab,
    req: ReviewerRequest,
) -> Result<(), serenity::Error> {
    match api::unassign_reviewer(
        installation,
        &req.owner,
        &req.repo_name,
        req.pr_number,
        &req.github_login,
    )
    .await
    {
        Ok(()) => {
            info!(req.pr_number, reviewer = %req.github_login, repo = %req.repo, "reviewer unassigned");
            post_reviewer_audit(
                ctx,
                app_state,
                &req.repo,
                req.pr_number,
                &cmd.user.name,
                &req.github_login,
                false,
            )
            .await;
            ephemeral(
                ctx,
                cmd,
                &format!(
                    "Removed review request from **{}** on PR #{} in **{}**.",
                    req.github_login, req.pr_number, req.repo,
                ),
            )
            .await
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to unassign reviewer");
            ephemeral(
                ctx,
                cmd,
                &format!(
                    "Could not remove the reviewer. Check the PR number and reviewer. \
                Also make sure {APP_NAME} is installed: {GITHUB_APP_URL}"
                ),
            )
            .await
        }
    }
}

/// Resolve `repo` and `pr_number` from the current thread or explicit options.
///
/// First checks if the command was run inside a PR audit thread — if so,
/// the context is inferred automatically. Falls back to the explicit `repo`
/// and `pr` options the user provided.
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
        Ok(Some((repo_opt.to_lowercase(), pr_number)))
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

/// Resolve a reviewer string to a GitHub login.
///
/// Accepts either a raw GitHub username or a Discord mention (`<@id>`).
/// For Discord mentions, looks up the linked GitHub login from the user store.
/// Returns `None` and sends an ephemeral error if resolution fails.
async fn resolve_reviewer(
    ctx: &Context,
    cmd: &CommandInteraction,
    app_state: &AppState,
    reviewer: &str,
) -> Result<Option<String>, serenity::Error> {
    if !reviewer.starts_with("<@") {
        return Ok(Some(reviewer.to_owned()));
    }

    let Some(discord_id) = reviewer
        .trim_start_matches("<@")
        .trim_end_matches('>')
        .parse::<u64>()
        .ok()
    else {
        ephemeral(ctx, cmd, "Could not parse that Discord mention.").await?;
        return Ok(None);
    };

    match app_state.user_store.get_by_discord(discord_id).await {
        Ok(Some(link)) => Ok(Some(link.github_login)),
        Ok(None) => {
            ephemeral(
                ctx,
                cmd,
                "That Discord user has not linked their GitHub account. \
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
}

/// Look up the PR audit thread and post a reviewer change entry.
async fn post_reviewer_audit(
    ctx: &Context,
    app_state: &AppState,
    repo: &str,
    pr_number: u64,
    actor: &str,
    reviewer: &str,
    assigned: bool,
) {
    match app_state
        .pr_store
        .get_all_by_repo_and_pr(repo, pr_number)
        .await
    {
        Ok(records) if records.is_empty() => {
            tracing::warn!(
                repo,
                pr_number,
                "no PR message records found for audit post"
            );
        }
        Ok(records) => {
            for record in records {
                if let Err(e) = messages::post_reviewer_change(
                    &ctx.http,
                    record.thread_id,
                    actor,
                    reviewer,
                    assigned,
                )
                .await
                {
                    tracing::error!(error = %e, "failed to post reviewer audit entry to thread");
                }
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to look up PR records for audit post");
        }
    }
}

/// Helper to fetch the `installation_id` and build the installation client.
async fn get_installation(
    app_state: &AppState,
    repo: &str,
    ctx: &Context,
    cmd: &CommandInteraction,
) -> Result<Option<Octocrab>, serenity::Error> {
    let installation_id = app_state
        .sub_store
        .get_installation_id(repo)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to look up installation ID");
            serenity::Error::Other("failed to look up installation")
        })?;

    let Some(id) = installation_id else {
        ephemeral(
            ctx,
            cmd,
            &format!("{APP_NAME} is not installed on that repository."),
        )
        .await?;
        return Ok(None);
    };

    let client = app_state.github.installation_client(id).map_err(|e| {
        tracing::error!(error = %e, "failed to build installation client");
        serenity::Error::Other("failed to build installation client")
    })?;

    Ok(Some(client))
}
