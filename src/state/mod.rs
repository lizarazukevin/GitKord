//! State persistence for `GitKord`.
//!
//! All access goes through the trait interfaces in [`traits`]
//! so the backing store can be swapped without touching handler code.

pub mod models;
pub mod sqlite;
pub mod traits;

pub use traits::{PrChannelMessageStore, SubscriptionStore, UserLinkStore};
