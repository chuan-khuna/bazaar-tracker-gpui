//! Everything Bazaar-specific with no UI. It must not depend on gpui.
//!
//! - [`log`] follows and parses `Player.log`.
//! - [`memory`] is the trait the run state machine reads game state through.
//! - [`run`] turns log lines and memory reads into Events.
//! - [`enums`] holds the game's enum values.
//! - [`history`] saves Runs and Events to `history.db`.
//! - [`catalog`] looks cards up in the game's card data.
//! - [`upload`] sends Runs to Bazaar DB, and keeps the secret key.
//!
//! Every module is a skeleton for now.

pub mod catalog;
pub mod enums;
pub mod history;
pub mod log;
pub mod memory;
pub mod run;
pub mod upload;
