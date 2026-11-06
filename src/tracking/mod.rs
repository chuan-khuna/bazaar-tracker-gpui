//! What the app knows of the tracker, with no drawing, grouped by what it feeds: the selected Run
//! (`run/`), the Live section, and the log panel (`log/`). At the top sit what they share: the
//! background service and how its notices reach the window, the connection state, the Runs and the
//! selection, card names and times. `ui/` draws them, in folders of the same names.

pub mod item_odds;
pub mod live;
pub mod log;
pub mod names;
pub mod odds_import;
pub mod run;
pub mod service;
pub mod session;
pub mod shop_odds;
pub mod status;
#[cfg(test)]
mod test_events;
pub mod when;
