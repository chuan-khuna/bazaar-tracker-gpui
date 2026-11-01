//! Reading the query of a request URL.

/// The path of `url`, without its scheme, host, query or fragment. `url` may also start at the path.
#[cfg_attr(not(feature = "webview"), allow(dead_code))]
pub(crate) fn path_of(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest.find('/').map_or("", |start| &rest[start..]));
    rest.split(['?', '#']).next().unwrap_or("")
}

/// The value of `name` in `url`'s query, as written (not percent-decoded). Empty if it is absent.
pub(crate) fn param(url: &str, name: &str) -> String {
    pairs(url).find(|(key, _)| *key == name).map_or_else(String::new, |(_, value)| value.to_owned())
}

/// `url`'s query without the `t` parameter: the page's signed, short-lived request token, which
/// says nothing about what was asked for.
pub(crate) fn without_token(url: &str) -> String {
    pairs(url).filter(|(key, _)| *key != "t").map(|(key, value)| format!("{key}={value}")).collect::<Vec<_>>().join("&")
}

fn pairs(url: &str) -> impl Iterator<Item = (&str, &str)> {
    let query = url.split_once('?').map_or("", |(_, query)| query.split('#').next().unwrap_or(""));
    query.split('&').filter(|pair| !pair.is_empty()).map(|pair| pair.split_once('=').unwrap_or((pair, "")))
}

#[cfg(test)]
#[path = "query_tests.rs"]
mod tests;
