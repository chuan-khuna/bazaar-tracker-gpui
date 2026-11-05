use bazaardb::{ItemOdds, ShopOdds};

use super::*;

#[test]
fn each_reply_is_listed_with_what_was_asked_and_how_it_went() {
    let shop = Seen::of(&Capture::ShopOdds(ShopOdds { days: bazaardb::shop_odds::defaults().days }));
    assert!(shop.ok && shop.label == "shop-odds" && shop.outcome.ends_with(" days"));
    let item = Seen::of(&Capture::ItemOdds(ItemOdds { item: "q1".into(), hero: "Jules".into(), tier: "None".into(), body: "{\"a\":1}".into() }));
    assert_eq!(item, Seen { ok: true, label: "item-odds · Jules · None · q1".into(), outcome: "7 B".into(), preview: "{\"a\":1}".into() });
    let failed = Seen::of(&Capture::Failed { api: Api::Runs, status: 403, why: "Bazaar DB answered HTTP 403".into() });
    assert_eq!((failed.ok, failed.label.as_str(), failed.outcome.as_str()), (false, "run", "Bazaar DB answered HTTP 403"));
}

#[test]
fn the_panel_keeps_the_newest_replies_first() {
    let (sender, _) = async_channel::unbounded();
    let mut panel = WebPanel::new(sender);
    for n in 0..KEPT + 5 {
        panel.push(Seen { ok: true, label: n.to_string(), outcome: String::new(), preview: String::new() });
    }
    assert_eq!(panel.seen.len(), KEPT);
    assert_eq!(panel.seen.front().unwrap().label, (KEPT + 4).to_string());
}

#[test]
fn bounds_become_physical_pixels() {
    let bounds = Bounds { origin: gpui::point(px(10.), px(20.5)), size: gpui::size(px(100.), px(50.)) };
    assert_eq!(physical(bounds, 1.5), Rect { x: 15, y: 31, width: 150, height: 75 });
}
