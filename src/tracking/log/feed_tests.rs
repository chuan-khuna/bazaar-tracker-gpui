use tracing_subscriber::layer::SubscriberExt;

use super::*;

fn rows(log: impl FnOnce()) -> Vec<LogRow> {
    let (layer, receiver) = channel();
    let subscriber = tracing_subscriber::registry().with(layer);
    tracing::subscriber::with_default(subscriber, log);
    std::iter::from_fn(|| receiver.try_recv().ok()).collect()
}

#[test]
fn a_record_becomes_a_row_with_its_level() {
    let rows = rows(|| {
        tracing::info!("data folder: {}", "C:\\data");
        tracing::warn!("something odd");
        tracing::error!("it broke");
    });
    let got: Vec<_> = rows.iter().map(|row| (row.kind, row.text.as_str())).collect();
    assert_eq!(got, [(Kind::Info, "data folder: C:\\data"), (Kind::Warn, "something odd"), (Kind::Error, "it broke")]);
    assert!(rows.iter().all(|row| row.time_ms > 0));
}

#[test]
fn fields_follow_the_message_in_plain_words() {
    let rows = rows(|| tracing::info!(pid = 42, reason = "gone", ok = false, "lost the game"));
    assert_eq!(rows[0].text, "lost the game (pid: 42, reason: gone, ok: false)");
}

#[test]
fn a_record_with_only_fields_is_still_a_row() {
    let rows = rows(|| tracing::info!(count = 3));
    assert_eq!(rows[0].text, "count: 3");
}

#[test]
fn records_made_with_nobody_listening_are_dropped_quietly() {
    let (layer, receiver) = channel();
    drop(receiver);
    let subscriber = tracing_subscriber::registry().with(layer);
    tracing::subscriber::with_default(subscriber, || tracing::info!("nobody hears this"));
}
