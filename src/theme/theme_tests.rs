use super::*;

fn names(query: &str) -> Vec<&'static str> {
    search(query).iter().map(|theme| theme.name).collect()
}

#[test]
fn there_are_ten_themes_and_rose_pine_is_first() {
    assert_eq!(THEMES.len(), 10);
    assert_eq!(THEMES[0].name, "Rosé Pine");
}

#[test]
fn every_theme_is_found_by_its_name() {
    for theme in THEMES {
        assert!(std::ptr::eq(by_name(Some(theme.name)), theme), "{}", theme.name);
    }
}

#[test]
fn an_unknown_or_missing_name_gives_rose_pine() {
    assert!(std::ptr::eq(by_name(Some("Solarized")), &ROSE_PINE));
    assert!(std::ptr::eq(by_name(Some("rosé pine")), &ROSE_PINE));
    assert!(std::ptr::eq(by_name(None), &ROSE_PINE));
}

#[test]
fn theme_names_are_unique() {
    let mut seen: Vec<&str> = THEMES.iter().map(|theme| theme.name).collect();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), THEMES.len());
}

#[test]
fn dark_themes_come_before_light_ones() {
    let first_light = THEMES.iter().position(|theme| !theme.dark).unwrap();
    assert!(THEMES[..first_light].iter().all(|theme| theme.dark));
    assert!(THEMES[first_light..].iter().all(|theme| !theme.dark));
    assert_eq!(first_light, 7);
}

#[test]
fn the_themes_are_in_the_agreed_order() {
    assert_eq!(
        THEMES.map(|theme| theme.name),
        [
            "Rosé Pine",
            "Catppuccin Mocha",
            "Flexoki Dark",
            "Tokyo Night",
            "Everforest",
            "Kanagawa Wave",
            "Nord",
            "Rosé Pine Dawn",
            "Catppuccin Latte",
            "Flexoki Light",
        ]
    );
}

#[test]
fn every_role_is_a_real_colour() {
    for theme in THEMES {
        for (role, colour) in theme.roles() {
            assert!(colour <= 0xffffff, "{} {role}", theme.name);
        }
    }
}

#[test]
fn rose_pine_maps_its_palette_onto_the_roles() {
    let foam = 0x9ccfd8;
    let t = &ROSE_PINE;
    assert!([t.win, t.live, t.heal, t.diamond].iter().all(|colour| *colour == foam));
    assert_eq!(t.loss, 0xeb6f92);
    assert_eq!(t.damage, t.loss);
    assert!([t.shield, t.value, t.gold].iter().all(|colour| *colour == 0xf6c177));
    assert_eq!(t.burn, 0xebbcba);
    assert_eq!(t.bronze, t.burn);
    assert_eq!(t.poison, 0x31748f);
    assert_eq!(t.regen, 0xc4a7e7);
    assert_eq!(t.legendary, t.regen);
    assert_eq!(t.ammo, t.silver);
}

#[test]
fn an_empty_query_lists_every_theme() {
    assert_eq!(names("").len(), THEMES.len());
}

#[test]
fn search_ignores_case_and_accents() {
    assert_eq!(names("ROSE"), ["Rosé Pine", "Rosé Pine Dawn"]);
    assert_eq!(names("rosé dawn"), ["Rosé Pine Dawn"]);
}

#[test]
fn search_matches_letters_in_order() {
    assert_eq!(names("cmo"), ["Catppuccin Mocha"]);
    assert_eq!(names("rose"), ["Rosé Pine", "Rosé Pine Dawn"]);
    assert_eq!(names("latte"), ["Catppuccin Latte"]);
    assert!(names("xyz").is_empty());
}

#[test]
fn search_finds_themes_by_appearance() {
    assert_eq!(names("light"), ["Rosé Pine Dawn", "Catppuccin Latte", "Flexoki Light"]);
    assert_eq!(names("dark").len(), 7);
}

#[test]
fn search_finds_the_other_themes() {
    assert_eq!(names("flexoki dark"), ["Flexoki Dark"]);
    assert_eq!(names("tokyo"), ["Tokyo Night"]);
    assert_eq!(names("everforest"), ["Everforest"]);
    assert_eq!(names("kanagawa"), ["Kanagawa Wave"]);
    assert_eq!(names("nord"), ["Nord"]);
}
