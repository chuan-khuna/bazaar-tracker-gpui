//! Runs that used a card, from `/api/run`.
//!
//! The answer's shape is not known yet, so it is kept as it came; a parser follows once a real
//! answer has been looked at.

use crate::query;

/// Where a card's page gets the runs that used it from.
pub const API_PATH: &str = "/api/run";

/// What `/api/run` answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Runs {
    /// What was asked for: the request's query, without the page's request token.
    pub query: String,
    /// The answer, unparsed.
    pub body: String,
}

impl Runs {
    /// The answer `body` to a request for `url`.
    pub fn from_reply(url: &str, body: String) -> Self {
        Self { query: query::without_token(url), body }
    }
}
