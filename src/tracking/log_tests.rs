use super::*;

fn row(n: i64) -> LogRow {
    LogRow { time_ms: n, kind: Kind::Info, text: format!("row {n}") }
}

#[test]
fn the_buffer_keeps_the_last_two_thousand_rows() {
    assert_eq!(CAPACITY, 2000);
    let mut buffer = LogBuffer::new();
    for n in 0..2500 {
        buffer.push(row(n));
    }
    assert_eq!(buffer.len(), 2000);
    let times: Vec<_> = buffer.rows().map(|row| row.time_ms).collect();
    assert_eq!((times[0], times[1999]), (500, 2499), "the oldest rows dropped off, the order kept");
}

#[test]
fn a_buffer_that_is_not_full_keeps_everything() {
    let mut buffer = LogBuffer::new();
    buffer.push(row(1));
    buffer.push(row(2));
    assert_eq!(buffer.rows().map(|row| row.time_ms).collect::<Vec<_>>(), [1, 2]);
}

#[test]
fn a_small_capacity_drops_exactly_the_oldest() {
    let mut buffer = LogBuffer::with_capacity(3);
    for n in 0..5 {
        buffer.push(row(n));
    }
    assert_eq!(buffer.rows().map(|row| row.time_ms).collect::<Vec<_>>(), [2, 3, 4]);
}

#[test]
fn kinds_have_words() {
    assert_eq!([Kind::Info, Kind::Warn, Kind::Error, Kind::Event].map(Kind::label), ["info", "warn", "error", "event"]);
}
