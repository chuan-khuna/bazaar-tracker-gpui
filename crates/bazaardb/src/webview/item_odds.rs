//! One item's odds, through the web view: the shop odds page asks for them when an item is picked,
//! or as it loads when its address names the item (`?item=<id>&hero=<hero>&tier=<tier>`).

use crate::ItemOdds;

/// The page to load, then pick an item on, for an item's odds.
pub const PAGE: &str = crate::SHOP_ODDS_PAGE;

/// The shop odds page set to `hero` and the owned `tier` (`None` when not owned), on the item with
/// Bazaar DB id `item` if it is known. Without it the player picks the item on the page.
pub fn page_for(item: Option<&str>, hero: &str, tier: &str) -> String {
    let mut pairs = Vec::new();
    if let Some(item) = item.filter(|item| !item.is_empty()) {
        pairs.push(format!("item={}", encode(item)));
    }
    if !hero.is_empty() {
        pairs.push(format!("hero={}", encode(hero)));
    }
    if !tier.is_empty() && tier != "None" {
        pairs.push(format!("tier={}", encode(tier)));
    }
    if pairs.is_empty() { PAGE.to_owned() } else { format!("{PAGE}?{}", pairs.join("&")) }
}

/// `value` with everything but letters, digits and `-._~` percent-encoded.
fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (byte as char).to_string(),
            other => format!("%{other:02X}"),
        })
        .collect()
}

/// Read a reply the page got from [`crate::item_odds::API_PATH`].
pub(super) fn read(url: &str, body: String) -> ItemOdds {
    ItemOdds::from_reply(url, body)
}

#[cfg(test)]
#[path = "item_odds_tests.rs"]
mod tests;
