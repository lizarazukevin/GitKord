//! Database abstraction layer.

mod postgres;

pub(crate) use postgres::create_stores;
