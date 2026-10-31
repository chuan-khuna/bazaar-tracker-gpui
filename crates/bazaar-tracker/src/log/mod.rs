//! Following `Player.log` from its end and parsing its lines into typed log lines.
//!
//! - [`follow`] reads new lines as the game writes them.
//! - [`parse`] turns a line into a [`LogLine`].
//! - [`path`] says where the log is.

pub mod follow;
pub mod parse;
pub mod path;

pub use follow::LogFollower;
pub use parse::{LogLine, parse_line};
