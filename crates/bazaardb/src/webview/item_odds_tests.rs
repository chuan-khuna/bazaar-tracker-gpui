use super::*;

#[test]
fn the_page_names_the_item_hero_and_tier() {
    assert_eq!(page_for(Some("qnwx879f"), "Jules", "Silver"), "https://bazaardb.gg/shop-odds?item=qnwx879f&hero=Jules&tier=Silver");
}

#[test]
fn an_unknown_item_or_an_unowned_tier_is_left_out() {
    assert_eq!(page_for(None, "Jules", "None"), "https://bazaardb.gg/shop-odds?hero=Jules");
    assert_eq!(page_for(Some(""), "", ""), "https://bazaardb.gg/shop-odds");
}

#[test]
fn values_are_encoded() {
    assert_eq!(page_for(None, "Big Hero&", ""), "https://bazaardb.gg/shop-odds?hero=Big%20Hero%26");
}

#[test]
fn the_page_to_find_an_item_names_it_in_its_fragment() {
    assert_eq!(page_to_find("Pasta Maker", "Jules", "Bronze"), "https://bazaardb.gg/shop-odds?hero=Jules&tier=Bronze#bazaar-tracker-find=Pasta%20Maker");
}
