//! Sending one request. A trait so the uploader's decisions can be tested without a network, and
//! one implementation over `ureq` for real use (and for the tests that run a fake server).

use std::time::Duration;

/// One batch to `POST`.
pub struct Request<'a> {
    pub url: &'a str,
    pub api_key: &'a str,
    pub user_agent: &'a str,
    /// The gzip-compressed JSON.
    pub body: &'a [u8],
}

/// The server's answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    /// The body as text (empty if it could not be read).
    pub body: String,
}

/// Why no answer came.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportError {
    /// The server took too long.
    Timeout,
    /// Anything else (no network, DNS, a refused connection, bad URL).
    Other(String),
}

pub trait Transport: Send + Sync {
    fn post(&self, request: &Request<'_>) -> Result<Response, TransportError>;
}

/// How long a request may take.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(110);

/// Sends with `ureq`. An HTTP error status is an answer, not an error.
pub struct UreqTransport {
    agent: ureq::Agent,
}

impl UreqTransport {
    pub fn new(timeout: Duration) -> Self {
        let config = ureq::Agent::config_builder().timeout_global(Some(timeout)).http_status_as_error(false).build();
        Self { agent: config.into() }
    }
}

impl Default for UreqTransport {
    fn default() -> Self {
        Self::new(REQUEST_TIMEOUT)
    }
}

impl Transport for UreqTransport {
    fn post(&self, request: &Request<'_>) -> Result<Response, TransportError> {
        let result = self
            .agent
            .post(request.url)
            .header("Content-Type", "application/json")
            .header("Content-Encoding", "gzip")
            .header("Authorization", format!("Bearer {}", request.api_key))
            .header("User-Agent", request.user_agent)
            .send(request.body);
        match result {
            Ok(mut response) => {
                let status = response.status().as_u16();
                let body = response.body_mut().read_to_string().unwrap_or_default();
                Ok(Response { status, body })
            }
            Err(ureq::Error::Timeout(_)) => Err(TransportError::Timeout),
            Err(error) => Err(TransportError::Other(error.to_string())),
        }
    }
}
