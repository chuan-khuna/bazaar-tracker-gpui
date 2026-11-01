//! One item's odds, through the web view: the shop odds page asks for them when an item is picked.

use crate::ItemOdds;

/// The page to load, then pick an item on, for an item's odds.
pub const PAGE: &str = crate::SHOP_ODDS_PAGE;

/// Read a reply the page got from [`crate::item_odds::API_PATH`].
pub(super) fn read(url: &str, body: String) -> ItemOdds {
    ItemOdds::from_reply(url, body)
}
