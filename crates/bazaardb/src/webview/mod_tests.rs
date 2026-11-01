use super::*;

fn message(url: &str, status: u16, body: &str) -> String {
    serde_json::json!({ "url": url, "status": status, "body": body }).to_string()
}

#[test]
fn each_api_url_names_its_api() {
    assert_eq!(Api::of_url("https://bazaardb.gg/api/shop-odds"), Some(Api::ShopOdds));
    assert_eq!(Api::of_url("/api/item-odds?item=a&hero=Jules"), Some(Api::ItemOdds));
    assert_eq!(Api::of_url("https://bazaardb.gg/api/run?sort=newest"), Some(Api::Runs));
    assert_eq!(Api::of_url("https://bazaardb.gg/api/runs"), None);
    assert_eq!(Api::of_url("https://bazaardb.gg/shop-odds"), None);
}

#[test]
fn a_shop_odds_reply_is_parsed() {
    let body = r#"{"days":[{"day":2,"samples":10,"tiers":{"Bronze":0.5,"Silver":0.5}},{"day":1,"tiers":{"bronze":1}}]}"#;
    let Some(Capture::ShopOdds(odds)) = decode(&message("https://bazaardb.gg/api/shop-odds", 200, body)) else { panic!("not shop odds") };
    assert_eq!(odds.days.iter().map(|day| day.day).collect::<Vec<_>>(), [1, 2]);
}

#[test]
fn an_item_odds_reply_keeps_what_was_asked_and_the_body() {
    let url = "https://bazaardb.gg/api/item-odds?item=q1&hero=Jules&tier=None&t=v2.1.a.b";
    let Some(Capture::ItemOdds(odds)) = decode(&message(url, 200, "{}")) else { panic!("not item odds") };
    assert_eq!(odds, ItemOdds { item: "q1".into(), hero: "Jules".into(), tier: "None".into(), body: "{}".into() });
}

#[test]
fn a_runs_reply_drops_the_request_token() {
    let url = "https://bazaardb.gg/api/run?sort=newest&skillCardIds=abc&t=v2.1.a.b";
    let Some(Capture::Runs(runs)) = decode(&message(url, 200, "[]")) else { panic!("not runs") };
    assert_eq!(runs.query, "sort=newest&skillCardIds=abc");
}

#[test]
fn a_refusal_or_a_challenge_is_a_failure_of_its_api() {
    let refused = decode(&message("https://bazaardb.gg/api/item-odds?item=a", 403, "")).unwrap();
    assert!(matches!(refused, Capture::Failed { api: Api::ItemOdds, status: 403, .. }));
    let challenge = decode(&message("https://bazaardb.gg/api/shop-odds", 200, "<title>Just a moment...</title>")).unwrap();
    assert!(matches!(challenge, Capture::Failed { api: Api::ShopOdds, status: 200, .. }));
    let unreadable = decode(&message("https://bazaardb.gg/api/shop-odds", 200, "not json")).unwrap();
    assert_eq!(unreadable.api(), Some(Api::ShopOdds));
    assert!(matches!(unreadable, Capture::Failed { .. }));
}

#[test]
fn other_messages_are_dropped() {
    assert!(decode("hello").is_none());
    assert!(decode(&message("https://bazaardb.gg/api/other", 200, "{}")).is_none());
    assert!(decode(r#"{"url":"https://bazaardb.gg/api/shop-odds"}"#).is_none());
}

#[test]
fn the_script_watches_every_api() {
    let script = script();
    assert!(!script.contains("__PATHS__"));
    for api in Api::ALL {
        assert!(script.contains(api.path()), "{api:?}");
    }
}

#[test]
fn a_find_report_is_read() {
    let message = |outcome: &str| serde_json::json!({ "find": "Pasta Maker", "outcome": outcome }).to_string();
    let Some(Capture::Find { name, outcome }) = decode(&message("picked")) else { panic!("not a find report") };
    assert_eq!((name.as_str(), outcome), ("Pasta Maker", FindOutcome::Picked));
    assert!(matches!(decode(&message("no-match")), Some(Capture::Find { outcome: FindOutcome::NoMatch, .. })));
    assert!(matches!(decode(&message("no-search-box")), Some(Capture::Find { outcome: FindOutcome::NoSearchBox, .. })));
    assert_eq!(decode(&message("picked")).unwrap().api(), None);
}

#[test]
fn the_script_knows_the_find_fragment() {
    let script = script();
    assert!(!script.contains("__FIND__"));
    assert!(script.contains(FIND_FRAGMENT));
}
