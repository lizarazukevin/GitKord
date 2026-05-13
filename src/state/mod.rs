//! State persistence for `DiGiBot`.
//!
//! Provides SQLite-backed stores for PR message IDs and Discord ↔ GitHub
//! user links. All access goes through the trait interfaces in [`traits`]
//! so the backing store can be swapped without touching handler code.

pub mod db;
pub mod traits;

#[allow(unused_imports)]
pub use traits::{
    PrMessage, PrMessageStore, Subscription, SubscriptionStore, UserLink, UserLinkStore,
};
