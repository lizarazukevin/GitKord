//! State persistence for `GitKord`.
//!
//! All access goes through the trait interfaces in [`traits`]
//! so the backing store can be swapped without touching handler code.

pub mod models;
pub mod postgres;
pub mod traits;

pub use traits::{PrChannelMessageStore, SubscriptionStore, UserLinkStore};
