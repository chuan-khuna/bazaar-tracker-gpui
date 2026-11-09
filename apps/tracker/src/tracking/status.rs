//! The connection state and the errors the status bar shows, worked out from the service's notices.

use bazaar_tracker::memory::ConnectionChange;
use bazaar_tracker::tracker::Notice;

/// Whether the tracker is following the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Connection {
    /// The service is not running (it could not start).
    NotTracking,
    /// The service is running, but the game is not attached.
    Waiting,
    /// The game is attached.
    Tracking,
}

impl Connection {
    pub fn label(self) -> &'static str {
        match self {
            Connection::NotTracking => "Not tracking",
            Connection::Waiting => "Waiting for the game",
            Connection::Tracking => "Tracking",
        }
    }
}

/// The connection, and which error was shown last.
#[derive(Debug)]
pub struct Status {
    pub connection: Connection,
    /// The last error handed out, so that the tracker repeating itself does not bring a dismissed
    /// error back. An Event being saved clears it: the problem, if any, is over.
    last_error: Option<String>,
}

impl Status {
    /// `running` says whether the service started.
    pub fn new(running: bool) -> Self {
        Self { connection: if running { Connection::Waiting } else { Connection::NotTracking }, last_error: None }
    }

    /// Take a notice in. Returns an error for the status bar to show, if the notice is a new one.
    pub fn apply(&mut self, notice: &Notice) -> Option<String> {
        match notice {
            Notice::Connection(ConnectionChange::Connected { .. }) => {
                self.connection = Connection::Tracking;
                None
            }
            Notice::Connection(ConnectionChange::Disconnected { .. }) => {
                self.connection = Connection::Waiting;
                None
            }
            Notice::EventSaved(_) | Notice::RunRebound { .. } => {
                self.last_error = None;
                None
            }
            Notice::SaveFailed { what, error, .. } => self.report(format!("Could not save {what}: {error}")),
            Notice::LogError(error) => self.report(format!("Could not read Player.log: {error}")),
            Notice::LiveSnapshot(_) | Notice::PickUp(_) => None,
        }
    }

    fn report(&mut self, text: String) -> Option<String> {
        if self.last_error.as_ref() == Some(&text) {
            return None;
        }
        self.last_error = Some(text.clone());
        Some(text)
    }
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
