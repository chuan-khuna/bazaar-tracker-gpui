use super::*;

#[test]
fn the_path_is_found_in_absolute_and_relative_urls() {
    assert_eq!(path_of("https://bazaardb.gg/api/shop-odds"), "/api/shop-odds");
    assert_eq!(path_of("/api/item-odds?item=a&hero=Jules"), "/api/item-odds");
    assert_eq!(path_of("https://bazaardb.gg/api/run?sort=newest#top"), "/api/run");
    assert_eq!(path_of("https://bazaardb.gg"), "");
}

#[test]
fn params_are_read_and_the_token_is_dropped() {
    let url = "https://bazaardb.gg/api/item-odds?item=q1&hero=Jules&tier=None&t=v2.1.a.b";
    assert_eq!(param(url, "hero"), "Jules");
    assert_eq!(param(url, "missing"), "");
    assert_eq!(without_token(url), "item=q1&hero=Jules&tier=None");
}
