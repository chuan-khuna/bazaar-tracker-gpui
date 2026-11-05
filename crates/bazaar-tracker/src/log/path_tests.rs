use super::*;

#[cfg(windows)]
#[test]
fn the_default_path_is_under_local_low() {
    let path = default_path_in(Path::new(r"C:\Users\player"));
    assert_eq!(path, Path::new(r"C:\Users\player\AppData\LocalLow\Tempo Storm\The Bazaar\Player.log"));
}

#[cfg(windows)]
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

#[cfg(target_os = "macos")]
#[test]
fn on_macos_the_log_and_the_card_data_are_in_different_folders() {
    let home = Path::new("/Users/player");
    assert_eq!(default_path_in(home), Path::new("/Users/player/Library/Logs/Tempo Storm/The Bazaar/Player.log"));
    assert_eq!(game_folder_in(home), Path::new("/Users/player/Library/Application Support/com.TempoStorm.TheBazaar"));
    assert_eq!(resolve("  /tmp/Player.log ", Some(home)), Path::new("/tmp/Player.log"));
}
