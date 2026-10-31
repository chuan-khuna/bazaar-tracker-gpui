use std::sync::mpsc;
use std::thread;

use super::*;

fn notice(text: &str) -> Notice {
    Notice::LogError(text.into())
}

#[test]
fn notices_arrive_in_order() {
    let (source, receiver) = mpsc::channel();
    let (out, notices) = async_channel::unbounded();
    let stop = AtomicBool::new(false);
    for text in ["a", "b", "c"] {
        source.send(notice(text)).unwrap();
    }
    drop(source);
    forward(&receiver, &out, &stop);
    drop(out);
    let got: Vec<_> = std::iter::from_fn(|| notices.try_recv().ok()).collect();
    assert_eq!(got, vec![notice("a"), notice("b"), notice("c")]);
}

#[test]
fn the_stop_flag_ends_it() {
    let (source, receiver) = mpsc::channel::<Notice>();
    let (out, _notices) = async_channel::unbounded();
    let stop = Arc::new(AtomicBool::new(false));
    let handle = {
        let stop = stop.clone();
        thread::spawn(move || forward(&receiver, &out, &stop))
    };
    stop.store(true, Ordering::Relaxed);
    handle.join().unwrap();
    drop(source);
}

#[test]
fn it_ends_when_nobody_is_listening() {
    let (source, receiver) = mpsc::channel();
    let (out, notices) = async_channel::unbounded();
    drop(notices);
    source.send(notice("a")).unwrap();
    forward(&receiver, &out, &AtomicBool::new(false));
}

#[test]
fn a_history_that_cannot_be_opened_gives_a_service_that_is_not_running() {
    let dir = tempfile::tempdir().unwrap();
    // A folder where the file should be.
    std::fs::create_dir(dir.path().join(bazaar_tracker::history::FILE_NAME)).unwrap();
    let service = Service::start(dir.path(), "");
    let link = service.link();
    assert!(!link.running());
    assert!(service.error.as_deref().is_some_and(|error| error.starts_with("Could not open history.db")));
    assert!(link.notices.try_recv().is_err());
}
