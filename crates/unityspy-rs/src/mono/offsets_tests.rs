use super::*;

#[test]
fn versions_parse_and_drop_the_editor_suffix() {
    assert_eq!(UnityVersion::parse("2022.3.42f1"), Some(UnityVersion::new(2022, 3, 42)));
    assert_eq!(UnityVersion::parse("6000.0.58"), Some(UnityVersion::new(6000, 0, 58)));
    assert_eq!(UnityVersion::parse("2021.3"), Some(UnityVersion::new(2021, 3, 0)));
    assert_eq!(UnityVersion::parse("nonsense"), None);
}

#[test]
fn the_table_is_chosen_by_the_unity_line() {
    assert_eq!(for_version(UnityVersion::new(2021, 3, 14)).unwrap().name, UNITY_2021_2022_X64.name);
    assert_eq!(for_version(UnityVersion::new(2022, 3, 42)).unwrap().name, UNITY_2021_2022_X64.name);
    let six = for_version(UnityVersion::new(6000, 0, 58)).unwrap();
    assert_eq!(six.name, UNITY_6_X64.name);
    assert!(!six.verified);
}

#[test]
fn an_unknown_unity_line_is_an_error_naming_the_version() {
    let error = for_version(UnityVersion::new(2019, 4, 5)).unwrap_err();
    assert!(matches!(error, Error::UnsupportedUnityVersion(ref v) if v == "2019.4.5"));
}

#[test]
fn the_2021_table_has_the_values_that_differ_from_2019() {
    assert_eq!(UNITY_2021_2022_X64.domain_assemblies, 0xA0);
    assert_eq!(UNITY_2021_2022_X64.image_class_cache, 0x4D0);
    assert_eq!(UNITY_2021_2022_X64.class_kind, 0x1B);
    assert_eq!(UNITY_2021_2022_X64.vtable, 0x48);
    // The expressions, not UnitySpy's stale comments (0xE0 and 0xE4).
    assert_eq!(UNITY_2021_2022_X64.class_field_count, 0x100);
    assert_eq!(UNITY_2021_2022_X64.class_next_class_cache, 0x108);
}
