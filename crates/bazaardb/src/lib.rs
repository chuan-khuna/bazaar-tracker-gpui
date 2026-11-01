//! Reading Bazaar DB (bazaardb.gg), starting with the shop odds.
//!
//! Fetching is a plain `GET` that names this app in its User-Agent. Bazaar DB's API wants a
//! Cloudflare Turnstile token that only a browser can earn, so without one the answer is a
//! refusal, which comes back as [`FetchError::Blocked`]; this crate does not work around that.
//! The caller can instead parse an answer saved from a browser with [`shop_odds::parse_json`].

pub mod fetch;
pub mod shop_odds;

pub use fetch::{FetchError, fetch_page};
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
