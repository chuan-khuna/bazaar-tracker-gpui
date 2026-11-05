//! The shop odds, through the web view: the shop odds page asks for them as it loads.

use crate::shop_odds::{self, ShopOdds};

/// The page to load for the shop odds.
pub const PAGE: &str = crate::SHOP_ODDS_PAGE;

/// Read a reply the page got from [`shop_odds::API_PATH`].
pub(super) fn read(body: &str) -> Result<ShopOdds, String> {
    shop_odds::parse_json(body).map_err(|error| error.to_string())
}
