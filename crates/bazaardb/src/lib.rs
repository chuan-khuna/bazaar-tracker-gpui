//! Reading Bazaar DB (bazaardb.gg).
//!
//! Each kind of data it serves has its own module, which holds its API path and what that path
//! answers with: [`shop_odds`], [`item_odds`] and [`runs`]. A reply reaches them one of two ways:
//!
//! - [`fetch`]: a plain `GET` that names this app in its User-Agent. Bazaar DB's API wants a
//!   Cloudflare Turnstile token that only a browser can earn, so without one the answer is a
//!   refusal, which comes back as [`FetchError::Blocked`]; this crate does not work around that.
//! - [`webview`] (the `webview` feature): Bazaar DB's own page in a real browser view, whose
//!   replies are read as they come back.
//!
//! An answer saved from a browser can also be parsed directly, as with [`shop_odds::parse_json`].

pub mod fetch;
pub mod item_odds;
mod query;
pub mod runs;
pub mod shop_odds;
#[cfg(feature = "webview")]
pub mod webview;

pub use fetch::{FetchError, fetch_page};
pub use item_odds::ItemOdds;
pub use runs::Runs;
pub use shop_odds::{DayOdds, ParseError, ShopOdds, TIERS};

/// The shop odds page, for people to open.
pub const SHOP_ODDS_PAGE: &str = "https://bazaardb.gg/shop-odds";

/// Where the page gets the odds from.
pub const SHOP_ODDS_API: &str = "https://bazaardb.gg/api/shop-odds";

/// Fetches the shop odds.
pub fn fetch_shop_odds() -> Result<ShopOdds, Error> {
    let text = fetch_page(SHOP_ODDS_API).map_err(Error::Fetch)?;
    shop_odds::parse_json(&text).map_err(Error::Parse)
}

/// Why no shop odds came back.
#[derive(Debug)]
pub enum Error {
    Fetch(FetchError),
    Parse(ParseError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Fetch(e) => e.fmt(f),
            Error::Parse(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {}
