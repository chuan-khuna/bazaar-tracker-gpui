//! The run state machine: log lines are triggers, memory reads give the data, Events come out.
//!
//! - [`event`] holds Runs and Events.

pub mod event;

pub use event::{Event, EventKind, Outcome, PresentedOption, Run};
