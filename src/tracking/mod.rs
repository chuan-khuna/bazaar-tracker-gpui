//! What the app knows of the tracker, with no drawing: the background service and how its notices
//! reach the window, the connection state, the Runs and the selection, and the view models of the
//! header, board, stash and skills. `ui/` draws them.

pub mod board;
pub mod header;
pub mod moment;
pub mod names;
pub mod service;
pub mod session;
pub mod status;
#[cfg(test)]
mod test_events;
pub mod when;
