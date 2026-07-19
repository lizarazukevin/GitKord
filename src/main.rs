#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]
#![allow(clippy::multiple_crate_versions)] // transitive deps — not in our control

//! `GitKord` entrypoint.
//!
//! Two tasks run concurrently for the lifetime of the process:
//! - Axum HTTP server — receives GitHub webhook payloads
//! - Serenity Discord client — handles gateway events and slash registry
//!
//! Both are spawned as Tokio tasks so neither blocks the other.
//! If either task exits unexpectedly, the process exits with a non-zero code
//! so Railway (or any supervisor) knows to restart it.

use git_kord::{init_tracing, run, AppError};

#[tokio::main]
async fn main() -> Result<(), AppError> {
    init_tracing();
    run().await
}
