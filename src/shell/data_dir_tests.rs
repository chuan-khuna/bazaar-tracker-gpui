use super::*;

const REPO: &str = "D:\\code\\bazaar-tracker-gpui";
const APP_DATA: &str = "C:\\Users\\player\\AppData\\Roaming";

fn dir(env: Option<&str>, debug: bool, app_data: Option<&str>) -> PathBuf {
    resolve(env.map(OsStr::new), debug, Path::new(REPO), app_data.map(Path::new))
}

#[test]
fn the_environment_variable_wins_in_any_build() {
    assert_eq!(dir(Some("E:\\scratch"), true, Some(APP_DATA)), PathBuf::from("E:\\scratch"));
    assert_eq!(dir(Some("E:\\scratch"), false, Some(APP_DATA)), PathBuf::from("E:\\scratch"));
}

#[test]
fn a_debug_build_uses_the_repos_folder() {
    assert_eq!(dir(None, true, Some(APP_DATA)), Path::new(REPO).join(".bazaar-tracker"));
}

#[test]
fn a_release_build_uses_app_data() {
    assert_eq!(dir(None, false, Some(APP_DATA)), Path::new(APP_DATA).join("BazaarTracker"));
}

#[test]
fn an_empty_variable_counts_as_unset() {
    assert_eq!(dir(Some(""), true, None), Path::new(REPO).join(".bazaar-tracker"));
    assert_eq!(dir(Some(""), false, Some(APP_DATA)), Path::new(APP_DATA).join("BazaarTracker"));
}

#[test]
fn a_release_build_without_app_data_still_gets_a_folder() {
    assert_eq!(dir(None, false, None), PathBuf::from("BazaarTracker"));
}

#[test]
fn the_debug_folder_is_in_the_repo() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(repo.join("Cargo.toml").exists());
    assert_eq!(resolve(None, true, repo, None), repo.join(".bazaar-tracker"));
}
