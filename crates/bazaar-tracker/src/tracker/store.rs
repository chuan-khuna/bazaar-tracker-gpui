//! Where the service saves what the run state machine produces.

use std::sync::Arc;

use crate::history::History;
use crate::run::Event;

/// A place Events are saved and Runs are rebound. Errors are text, since the service only reports
/// them and tries again.
pub trait Store: Send {
    fn save_event(&mut self, event: &Event) -> Result<(), String>;
    fn rebind_run(&mut self, old_run_id: &str, new_run_id: &str) -> Result<(), String>;
}

impl Store for Arc<History> {
    fn save_event(&mut self, event: &Event) -> Result<(), String> {
        self.insert_event(event).map(|_| ()).map_err(|error| error.to_string())
    }

    fn rebind_run(&mut self, old_run_id: &str, new_run_id: &str) -> Result<(), String> {
        self.as_ref().rebind_run(old_run_id, new_run_id).map(|_| ()).map_err(|error| error.to_string())
    }
}
