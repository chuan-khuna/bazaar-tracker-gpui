use super::*;

/// A scratch folder that is removed when it goes out of scope.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("bazaar-tracker-settings-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn file(&self) -> PathBuf {
        self.0.join(FILE_NAME)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn the_defaults_are_the_specs() {
    let settings = Settings::default();
    assert_eq!(settings.theme, "Rosé Pine");
    assert_eq!(settings.log_path, "");
    assert!(!settings.run_sync, "uploading is off until the player turns it on");
    assert!(!settings.show_run_duration);
    assert_eq!(settings.window, WindowSettings { sidebar_open: true, sidebar_width: 248, detail_open: true, detail_width: 300, stash_open: true, log_open: false, log_details_open: true, log_width: 420 });
}

#[test]
fn an_empty_file_gives_the_defaults() {
    let dir = Scratch::new("empty");
    fs::write(dir.file(), "").unwrap();
    let (store, outcome) = SettingsStore::load(&dir.0);
    assert_eq!(outcome, Load::Read);
    assert_eq!(store.get(), &Settings::default());
}

#[test]
fn a_missing_file_gives_the_defaults_and_is_created() {
    let dir = Scratch::new("missing");
    let (store, outcome) = SettingsStore::load(&dir.0);
    assert_eq!(outcome, Load::Created);
    assert_eq!(store.get(), &Settings::default());
    let (again, outcome) = SettingsStore::load(&dir.0);
    assert_eq!(outcome, Load::Read);
    assert_eq!(again.get(), &Settings::default());
}

#[test]
fn a_partial_file_keeps_the_keys_it_gives() {
    let dir = Scratch::new("partial");
    fs::write(dir.file(), "theme = \"Nord\"\nshow_run_duration = true\n\n[window]\nsidebar_open = false\n").unwrap();
    let (store, outcome) = SettingsStore::load(&dir.0);
    assert_eq!(outcome, Load::Read);
    let settings = store.get();
    assert_eq!(settings.theme, "Nord");
    assert!(settings.show_run_duration);
    assert!(!settings.window.sidebar_open);
    // What the file lacks has its default.
    assert!(!settings.run_sync);
    assert_eq!(settings.window.sidebar_width, SIDEBAR_DEFAULT_WIDTH);
    assert!(settings.window.detail_open);
}

#[test]
fn unknown_keys_are_ignored() {
    let dir = Scratch::new("unknown");
    fs::write(dir.file(), "theme = \"Nord\"\nfuture_option = 3\n\n[window]\nsomething_new = true\ndetail_open = false\n\n[other]\nx = 1\n").unwrap();
    let (store, outcome) = SettingsStore::load(&dir.0);
    assert_eq!(outcome, Load::Read);
    assert_eq!(store.get().theme, "Nord");
    assert!(!store.get().window.detail_open);
}

#[test]
fn an_unparsable_file_gives_the_defaults_and_is_left_untouched() {
    let dir = Scratch::new("unparsable");
    let text = "theme = \n this is [not toml";
    fs::write(dir.file(), text).unwrap();
    let (store, outcome) = SettingsStore::load(&dir.0);
    assert!(matches!(outcome, Load::Unusable(_)));
    assert_eq!(store.get(), &Settings::default());
    assert_eq!(fs::read_to_string(dir.file()).unwrap(), text);
}

#[test]
fn an_unparsable_file_is_replaced_once_a_setting_changes() {
    let dir = Scratch::new("replaced");
    fs::write(dir.file(), "= broken").unwrap();
    let (mut store, _) = SettingsStore::load(&dir.0);
    store.update(|settings| settings.show_run_duration = true).unwrap();
    let (reloaded, outcome) = SettingsStore::load(&dir.0);
    assert_eq!(outcome, Load::Read);
    assert!(reloaded.get().show_run_duration);
}

#[test]
fn saving_then_loading_round_trips() {
    let dir = Scratch::new("round-trip");
    let (mut store, _) = SettingsStore::load(&dir.0);
    store
        .update(|settings| {
            settings.theme = "Catppuccin Latte".into();
            settings.log_path = "C:\\Games\\Player.log".into();
            settings.run_sync = true;
            settings.show_run_duration = true;
            settings.window = WindowSettings { sidebar_open: false, sidebar_width: 300, detail_open: false, detail_width: 360, stash_open: false, log_open: true, log_details_open: false, log_width: 300 };
        })
        .unwrap();
    let (reloaded, outcome) = SettingsStore::load(&dir.0);
    assert_eq!(outcome, Load::Read);
    assert_eq!(reloaded.get(), store.get());
    assert_eq!(reloaded.get().theme, "Catppuccin Latte");
    assert_eq!(reloaded.get().window.sidebar_width, 300);
}

#[test]
fn the_saved_file_uses_the_documented_keys() {
    let dir = Scratch::new("keys");
    let (mut store, _) = SettingsStore::load(&dir.0);
    store.update(|settings| settings.show_run_duration = true).unwrap();
    let text = fs::read_to_string(dir.file()).unwrap();
    for key in ["theme", "log_path", "run_sync", "show_run_duration", "[window]", "sidebar_open", "sidebar_width", "detail_open", "detail_width", "stash_open", "log_open", "log_details_open", "log_width"] {
        assert!(text.contains(key), "{key} is missing from\n{text}");
    }
}

#[test]
fn an_update_that_changes_nothing_does_not_write() {
    let dir = Scratch::new("no-change");
    let text = "= broken";
    fs::write(dir.file(), text).unwrap();
    let (mut store, _) = SettingsStore::load(&dir.0);
    store.update(|_| {}).unwrap();
    assert_eq!(fs::read_to_string(dir.file()).unwrap(), text);
}

#[test]
fn the_sidebar_width_is_clamped_at_both_ends() {
    let dir = Scratch::new("clamp");
    let (mut store, _) = SettingsStore::load(&dir.0);

    store.update(|settings| settings.window.set_sidebar_width(10)).unwrap();
    assert_eq!(store.get().window.sidebar_width, SIDEBAR_MIN_WIDTH);
    store.update(|settings| settings.window.set_sidebar_width(9000)).unwrap();
    assert_eq!(store.get().window.sidebar_width, SIDEBAR_MAX_WIDTH);
    store.update(|settings| settings.window.set_sidebar_width(300)).unwrap();
    assert_eq!(store.get().window.sidebar_width, 300);

    // What was saved is within range too.
    store.update(|settings| settings.window.sidebar_width = 1).unwrap();
    let (reloaded, _) = SettingsStore::load(&dir.0);
    assert_eq!(reloaded.get().window.sidebar_width, SIDEBAR_MIN_WIDTH);
}

#[test]
fn a_hand_edited_width_out_of_range_is_clamped_on_load() {
    let dir = Scratch::new("clamp-load");
    fs::write(dir.file(), "[window]\nsidebar_width = 5\n").unwrap();
    assert_eq!(SettingsStore::load(&dir.0).0.get().window.sidebar_width, SIDEBAR_MIN_WIDTH);
    fs::write(dir.file(), "[window]\nsidebar_width = 5000\n").unwrap();
    assert_eq!(SettingsStore::load(&dir.0).0.get().window.sidebar_width, SIDEBAR_MAX_WIDTH);
}

#[test]
fn the_log_width_is_clamped_at_both_ends_and_on_load() {
    let dir = Scratch::new("log-width");
    let (mut store, _) = SettingsStore::load(&dir.0);
    store.update(|settings| settings.window.set_log_width(1)).unwrap();
    assert_eq!(store.get().window.log_width, LOG_MIN_WIDTH);
    store.update(|settings| settings.window.set_log_width(100_000)).unwrap();
    assert_eq!(store.get().window.log_width, LOG_MAX_WIDTH);
    fs::write(dir.file(), "[window]\nlog_width = 5\n").unwrap();
    let (loaded, _) = SettingsStore::load(&dir.0);
    assert_eq!(loaded.get().window.log_width, LOG_MIN_WIDTH);
    assert!(!loaded.get().window.log_open);
}

#[test]
fn the_detail_width_is_clamped_at_both_ends_and_on_load() {
    let dir = Scratch::new("detail-width");
    let (mut store, _) = SettingsStore::load(&dir.0);
    store.update(|settings| settings.window.set_detail_width(1)).unwrap();
    assert_eq!(store.get().window.detail_width, DETAIL_MIN_WIDTH);
    store.update(|settings| settings.window.set_detail_width(100_000)).unwrap();
    assert_eq!(store.get().window.detail_width, DETAIL_MAX_WIDTH);
    fs::write(dir.file(), "[window]\ndetail_width = 5\n").unwrap();
    let (loaded, _) = SettingsStore::load(&dir.0);
    assert_eq!(loaded.get().window.detail_width, DETAIL_MIN_WIDTH);
}
