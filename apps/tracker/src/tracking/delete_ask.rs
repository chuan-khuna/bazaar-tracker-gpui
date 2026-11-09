//! Which row of a list is asking "Delete this?": the Runs in the sidebar, the Conversations in the
//! Advisor tab. A row's delete icon opens a strip under it with Cancel and Delete; nothing is
//! deleted until Delete is clicked. At most one row asks at a time.

/// The row (by id) whose delete strip is open, if any.
#[derive(Debug, Default)]
pub struct DeleteAsk(Option<String>);

impl DeleteAsk {
    /// The delete icon was clicked: this row asks, and any other row stops asking.
    pub fn ask(&mut self, id: &str) {
        self.0 = Some(id.to_owned());
    }

    /// Cancel or Escape: no row asks. Whether one was asking, so Escape is used up only then.
    pub fn cancel(&mut self) -> bool {
        self.0.take().is_some()
    }

    /// A row was clicked: asking stops unless it is the row that asks.
    pub fn clicked(&mut self, id: &str) {
        if !self.is(id) {
            self.0 = None;
        }
    }

    /// Delete was clicked on this row: whether it was asking, which is when it may be deleted.
    pub fn confirm(&mut self, id: &str) -> bool {
        let asking = self.is(id);
        if asking {
            self.0 = None;
        }
        asking
    }

    pub fn asking(&self) -> Option<&str> {
        self.0.as_deref()
    }

    pub fn is(&self, id: &str) -> bool {
        self.asking() == Some(id)
    }
}

#[cfg(test)]
#[path = "delete_ask_tests.rs"]
mod tests;
