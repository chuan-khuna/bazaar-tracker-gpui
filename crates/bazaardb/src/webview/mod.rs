//! Bazaar DB's own pages in an Edge (WebView2) view, with each reply they get from a watched API
//! handed back already read.
//!
//! The page loads in a real browser on the player's machine, so Cloudflare's check passes as in any
//! browser, and the page signs its own requests. A script added to the page copies the replies;
//! nothing here sends a request of its own.
//!
//! Each API has a module here for what is particular to getting it this way: [`shop_odds`],
//! [`item_odds`] and [`runs`]. Reading the replies builds on every platform; the view itself
//! ([`WebView`]) is Windows only.

pub mod item_odds;
pub mod runs;
pub mod shop_odds;
#[cfg(windows)]
mod view;

#[cfg(windows)]
pub use view::{Rect, WebView};

use crate::fetch::is_challenge;
use crate::query;
use crate::{ItemOdds, Runs, ShopOdds};

/// An API the page script watches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Api {
    ShopOdds,
    ItemOdds,
    Runs,
}

impl Api {
    pub const ALL: [Api; 3] = [Api::ShopOdds, Api::ItemOdds, Api::Runs];

    pub fn path(self) -> &'static str {
        match self {
            Api::ShopOdds => crate::shop_odds::API_PATH,
            Api::ItemOdds => crate::item_odds::API_PATH,
            Api::Runs => crate::runs::API_PATH,
        }
    }

    /// Which API `url` is a request to, by its path alone.
    pub fn of_url(url: &str) -> Option<Api> {
        let path = query::path_of(url);
        Api::ALL.into_iter().find(|api| api.path() == path)
    }
}

/// One reply the page got, read.
#[derive(Debug)]
pub enum Capture {
    ShopOdds(ShopOdds),
    ItemOdds(ItemOdds),
    Runs(Runs),
    /// A reply from a watched API that was refused or could not be read.
    Failed { api: Api, status: u16, why: String },
}

impl Capture {
    /// Which API the reply came from.
    pub fn api(&self) -> Api {
        match self {
            Capture::ShopOdds(_) => Api::ShopOdds,
            Capture::ItemOdds(_) => Api::ItemOdds,
            Capture::Runs(_) => Api::Runs,
            Capture::Failed { api, .. } => *api,
        }
    }
}

/// The page script, with the API paths it watches filled in.
pub fn script() -> String {
    let paths: Vec<&str> = Api::ALL.iter().map(|api| api.path()).collect();
    include_str!("script.js").replace("__PATHS__", &serde_json::to_string(&paths).unwrap_or_else(|_| "[]".into()))
}

/// Read one message from the page script: `{ url, status, body }`. `None` if it is not one, or its
/// URL is not a watched API.
pub fn decode(message: &str) -> Option<Capture> {
    let value: serde_json::Value = serde_json::from_str(message).ok()?;
    let url = value.get("url")?.as_str()?;
    let status = u16::try_from(value.get("status")?.as_u64()?).ok()?;
    let body = value.get("body").and_then(|body| body.as_str()).unwrap_or_default().to_owned();
    let api = Api::of_url(url)?;
    let failed = |why: String| Capture::Failed { api, status, why };
    if is_challenge(&body) {
        return Some(failed("Bazaar DB's bot check answered instead".into()));
    }
    if !(200..300).contains(&status) {
        return Some(failed(format!("Bazaar DB answered HTTP {status}")));
    }
    Some(match api {
        Api::ShopOdds => shop_odds::read(&body).map_or_else(failed, Capture::ShopOdds),
        Api::ItemOdds => Capture::ItemOdds(item_odds::read(url, body)),
        Api::Runs => Capture::Runs(runs::read(url, body)),
    })
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
