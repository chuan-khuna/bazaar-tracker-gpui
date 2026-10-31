//! The run state machine: log lines are triggers, memory reads give the data, Events come out.
//!
//! - [`event`] holds Runs and Events.
//! - [`machine`] is the [`RunMachine`] itself; [`lifecycle`], [`options`] and [`combat`] are its parts.
//! - [`clock`] is the time it runs on, which tests move by hand.

pub mod clock;
mod combat;
pub mod event;
mod lifecycle;
pub mod machine;
mod options;
#[cfg(test)]
mod test_support;

pub use clock::{Clock, ManualClock, SystemClock};
pub use event::{Event, EventKind, Outcome, PresentedOption, Run, is_local_run_id};
pub use machine::{Output, RunMachine};
