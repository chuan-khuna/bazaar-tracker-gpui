use super::*;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bazaar-tracker-logging-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

#[test]
fn lines_are_appended_to_the_file() {
    let dir = scratch("append");
    let path = dir.join("logs").join(FILE_NAME);
    let mut log = RotatingFile::open(path.clone(), 1000, 2).unwrap();
    log.write_all(b"one\n").unwrap();
    log.write_all(b"two\n").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "one\ntwo\n");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_full_file_moves_to_a_backup() {
    let dir = scratch("rotate");
    let path = dir.join(FILE_NAME);
    let mut log = RotatingFile::open(path.clone(), 10, 2).unwrap();
    log.write_all(b"aaaaaa\n").unwrap();
    log.write_all(b"bbbbbb\n").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "bbbbbb\n");
    assert_eq!(fs::read_to_string(dir.join("tracker.log.1")).unwrap(), "aaaaaa\n");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn only_a_few_backups_are_kept() {
    let dir = scratch("backups");
    let path = dir.join(FILE_NAME);
    let mut log = RotatingFile::open(path.clone(), 10, 2).unwrap();
    for line in ["aaaaaa\n", "bbbbbb\n", "cccccc\n", "dddddd\n"] {
        log.write_all(line.as_bytes()).unwrap();
    }
    assert_eq!(fs::read_to_string(&path).unwrap(), "dddddd\n");
    assert_eq!(fs::read_to_string(dir.join("tracker.log.1")).unwrap(), "cccccc\n");
    assert_eq!(fs::read_to_string(dir.join("tracker.log.2")).unwrap(), "bbbbbb\n");
    assert!(!dir.join("tracker.log.3").exists());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn an_existing_file_keeps_growing_across_launches() {
    let dir = scratch("reopen");
    let path = dir.join(FILE_NAME);
    RotatingFile::open(path.clone(), 100, 1).unwrap().write_all(b"first launch\n").unwrap();
    RotatingFile::open(path.clone(), 100, 1).unwrap().write_all(b"second launch\n").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "first launch\nsecond launch\n");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_banner_has_the_version_and_process_id() {
    let banner = banner();
    assert!(banner.contains(env!("CARGO_PKG_VERSION")));
    assert!(banner.contains(&std::process::id().to_string()));
}
