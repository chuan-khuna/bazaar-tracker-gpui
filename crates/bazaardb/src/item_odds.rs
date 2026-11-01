//! One item's odds, from `/api/item-odds`.
//!
//! The answer's shape is not known yet, so it is kept as it came; a parser follows once a real
//! answer has been looked at.

use crate::query;

/// Where the shop odds page gets one item's odds from.
pub const API_PATH: &str = "/api/item-odds";

/// What `/api/item-odds` answered for one item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemOdds {
    /// The item's Bazaar DB id, the hero and the tier asked about, from the request.
    pub item: String,
    pub hero: String,
    pub tier: String,
    /// The answer, unparsed.
    pub body: String,
}

impl ItemOdds {
    /// The answer `body` to a request for `url`.
    pub fn from_reply(url: &str, body: String) -> Self {
        Self { item: query::param(url, "item"), hero: query::param(url, "hero"), tier: query::param(url, "tier"), body }
    }
}
