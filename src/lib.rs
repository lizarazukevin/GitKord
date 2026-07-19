//! `GitKord`: a `Discord` bot that brings `GitHub` PR notifications and
//! reviewer management directly into your server.

mod app;
mod config;
mod db;
mod discord;
mod error;
mod github;
mod models;
mod service;

pub use app::{init_tracing, run};
pub use error::AppError;
