use super::*;

#[test]
fn a_challenge_page_is_blocked_whatever_its_status() {
    let body = "<html><head><title>Just a moment...</title></head></html>".to_string();
    assert_eq!(classify(503, false, body.clone()), Err(FetchError::Blocked { status: 503 }));
    assert_eq!(classify(200, false, body), Err(FetchError::Blocked { status: 200 }));
}

#[test]
fn the_cf_mitigated_header_or_a_403_is_blocked() {
    assert_eq!(classify(200, true, String::new()), Err(FetchError::Blocked { status: 200 }));
    assert_eq!(classify(403, false, String::new()), Err(FetchError::Blocked { status: 403 }));
}

#[test]
fn other_failures_keep_their_status() {
    assert_eq!(classify(500, false, "oops".into()), Err(FetchError::Status(500)));
}

#[test]
fn a_success_returns_the_body() {
    assert_eq!(classify(200, false, "<table></table>".into()), Ok("<table></table>".into()));
}
