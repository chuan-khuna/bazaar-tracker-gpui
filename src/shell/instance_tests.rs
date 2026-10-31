use super::*;

fn unique(name: &str) -> String {
    format!("BazaarTrackerTest-{}-{name}", std::process::id())
}

#[test]
fn the_name_depends_on_the_folder_only_up_to_case_and_slashes() {
    let a = name_for(Path::new(r"C:\Users\Player\AppData\Roaming\BazaarTracker"));
    assert_eq!(a, name_for(Path::new("c:/users/player/appdata/roaming/bazaartracker/")));
    assert_ne!(a, name_for(Path::new(r"C:\scratch")));
    assert!(a.starts_with("BazaarTracker-"));
}

#[cfg(windows)]
#[test]
fn a_second_launch_asks_the_first_to_show_its_window_and_exits() {
    let name = unique("second");
    let Launch::First(guard) = acquire_named(&name) else { panic!("the first launch should get the lock") };
    let requests = guard.show_requests();
    assert!(requests.is_empty());

    assert!(matches!(acquire_named(&name), Launch::Second));
    requests.recv_blocking().expect("the first copy hears the second");
}

#[cfg(windows)]
#[test]
fn copies_with_different_names_do_not_get_in_each_others_way() {
    let first = acquire_named(&unique("a"));
    let other = acquire_named(&unique("b"));
    assert!(matches!(first, Launch::First(_)));
    assert!(matches!(other, Launch::First(_)));
}

#[cfg(windows)]
#[test]
fn the_lock_is_free_again_once_the_first_copy_is_gone() {
    let name = unique("released");
    drop(acquire_named(&name));
    assert!(matches!(acquire_named(&name), Launch::First(_)));
}
