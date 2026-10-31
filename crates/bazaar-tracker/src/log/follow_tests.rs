use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::mpsc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use super::*;

fn append(path: &Path, text: &str) {
    let mut file = OpenOptions::new().append(true).create(true).open(path).unwrap();
    file.write_all(text.as_bytes()).unwrap();
}

fn log() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Player.log");
    (dir, path)
}

#[test]
fn it_starts_at_the_end_so_an_old_session_is_not_replayed() {
    let (_dir, path) = log();
    append(&path, "old one\nold two\n");
    let mut follower = LogFollower::new(&path);
    assert!(follower.poll().unwrap().is_empty());
    assert!(follower.poll().unwrap().is_empty());
    append(&path, "new one\n");
    assert_eq!(follower.poll().unwrap(), ["new one"]);
}

#[test]
fn lines_arrive_in_order_across_passes() {
    let (_dir, path) = log();
    append(&path, "x\n");
    let mut follower = LogFollower::new(&path);
    follower.poll().unwrap();
    append(&path, "a\nb\n");
    assert_eq!(follower.poll().unwrap(), ["a", "b"]);
    append(&path, "c\n");
    assert_eq!(follower.poll().unwrap(), ["c"]);
    assert!(follower.poll().unwrap().is_empty());
}

#[test]
fn a_partial_last_line_is_held_back_until_it_is_complete() {
    let (_dir, path) = log();
    append(&path, "x\n");
    let mut follower = LogFollower::new(&path);
    follower.poll().unwrap();
    append(&path, "whole\npart");
    assert_eq!(follower.poll().unwrap(), ["whole"]);
    assert!(follower.poll().unwrap().is_empty());
    append(&path, "ial\n");
    assert_eq!(follower.poll().unwrap(), ["partial"]);
}

#[test]
fn line_endings_are_removed_and_blank_lines_dropped() {
    let (_dir, path) = log();
    append(&path, "x\n");
    let mut follower = LogFollower::new(&path);
    follower.poll().unwrap();
    append(&path, "a\r\n\r\n   \r\nb\r\n");
    assert_eq!(follower.poll().unwrap(), ["a", "b"]);
}

#[test]
fn a_missing_file_is_waited_for_and_then_read_from_its_first_line() {
    let (_dir, path) = log();
    let mut follower = LogFollower::new(&path);
    assert!(follower.poll().unwrap().is_empty());
    assert!(follower.poll().unwrap().is_empty());
    // The game started after the tracker: everything in its new log is new.
    append(&path, "first\nsecond\n");
    assert_eq!(follower.poll().unwrap(), ["first", "second"]);
    append(&path, "third\n");
    assert_eq!(follower.poll().unwrap(), ["third"]);
}

#[test]
fn a_truncated_file_is_read_again_from_the_start() {
    let (_dir, path) = log();
    append(&path, "a long first session line one\na long first session line two\n");
    let mut follower = LogFollower::new(&path);
    follower.poll().unwrap();
    append(&path, "late\n");
    assert_eq!(follower.poll().unwrap(), ["late"]);
    // The game restarts and writes a shorter log in place.
    fs::write(&path, "n1\nn2\n").unwrap();
    assert_eq!(follower.poll().unwrap(), ["n1", "n2"]);
    append(&path, "n3\n");
    assert_eq!(follower.poll().unwrap(), ["n3"]);
}

#[test]
fn a_replaced_file_that_is_already_longer_is_detected_by_its_tail() {
    let (_dir, path) = log();
    append(&path, "session one line a\nsession one line b\n");
    let mut follower = LogFollower::new(&path);
    follower.poll().unwrap();
    // A different, longer file with the same name: only the tail signature can tell.
    let replacement = "session two line one is longer\nsession two line two is longer\nsession two line three is longer\n";
    fs::write(&path, replacement).unwrap();
    assert_eq!(follower.poll().unwrap(), ["session two line one is longer", "session two line two is longer", "session two line three is longer"]);
}

#[test]
fn a_file_deleted_and_recreated_is_followed_again() {
    let (_dir, path) = log();
    append(&path, "old\n");
    let mut follower = LogFollower::new(&path);
    follower.poll().unwrap();
    fs::remove_file(&path).unwrap();
    assert!(follower.poll().unwrap().is_empty());
    append(&path, "a brand new session begins here\n");
    assert_eq!(follower.poll().unwrap(), ["a brand new session begins here"]);
}

#[test]
fn at_most_sixteen_mebibytes_are_taken_in_one_pass() {
    let (_dir, path) = log();
    append(&path, "x\n");
    let mut follower = LogFollower::new(&path);
    follower.poll().unwrap();
    let line = format!("{}\n", "y".repeat(99));
    let count = 180_000;
    append(&path, &line.repeat(count));
    let first = follower.poll().unwrap();
    assert!(first.len() * 100 <= MAX_PASS_BYTES as usize, "took {} lines", first.len());
    assert!(first.len() < count, "everything came in one pass");
    let second = follower.poll().unwrap();
    assert_eq!(first.len() + second.len(), count);
}

#[test]
fn a_multibyte_character_split_across_writes_is_decoded_whole() {
    let (_dir, path) = log();
    append(&path, "x\n");
    let mut follower = LogFollower::new(&path);
    follower.poll().unwrap();
    let bytes = "Rosé\n".as_bytes();
    // Write only the first byte of the two-byte "é" and the rest later.
    let mut file = OpenOptions::new().append(true).open(&path).unwrap();
    file.write_all(&bytes[..4]).unwrap();
    assert!(follower.poll().unwrap().is_empty());
    file.write_all(&bytes[4..]).unwrap();
    assert_eq!(follower.poll().unwrap(), ["Rosé"]);
}

#[test]
fn running_delivers_lines_until_stopped() {
    let (_dir, path) = log();
    append(&path, "x\n");
    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::channel();
    let thread = {
        let stop = stop.clone();
        let path = path.clone();
        std::thread::spawn(move || LogFollower::new(path).run(&stop, |lines| sender.send(lines).unwrap(), |error| panic!("{error}")))
    };
    // Give the follower time to take its starting position before writing.
    std::thread::sleep(Duration::from_millis(150));
    append(&path, "hello\n");
    assert_eq!(receiver.recv_timeout(Duration::from_secs(5)).unwrap(), ["hello"]);
    stop.store(true, Ordering::Relaxed);
    thread.join().unwrap();
}
