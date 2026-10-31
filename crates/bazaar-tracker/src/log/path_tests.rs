use super::*;

#[test]
fn the_default_path_is_under_local_low() {
    let path = default_path_in(Path::new(r"C:\Users\player"));
    assert_eq!(path, Path::new(r"C:\Users\player\AppData\LocalLow\Tempo Storm\The Bazaar\Player.log"));
}

#[test]
fn the_setting_overrides_the_default() {
    let profile = Path::new(r"C:\Users\player");
    assert_eq!(resolve(r"D:\logs\Player.log", Some(profile)), Path::new(r"D:\logs\Player.log"));
    assert_eq!(resolve(r"  D:\logs\Player.log  ", Some(profile)), Path::new(r"D:\logs\Player.log"));
}

#[test]
fn an_empty_or_blank_setting_means_the_default() {
    let profile = Path::new(r"C:\Users\player");
    assert_eq!(resolve("", Some(profile)), default_path_in(profile));
    assert_eq!(resolve("   ", Some(profile)), default_path_in(profile));
}

#[test]
fn with_no_profile_there_is_only_a_bare_file_name() {
    assert_eq!(resolve("", None), Path::new("Player.log"));
}
