//! Runs that used a card, through the web view: a card's page asks for them as it loads.

use crate::Runs;

/// Read a reply the page got from [`crate::runs::API_PATH`].
pub(super) fn read(url: &str, body: String) -> Runs {
    Runs::from_reply(url, body)
}
