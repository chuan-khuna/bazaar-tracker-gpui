//! One honest `GET` for a page.

use std::time::Duration;

/// Why no page came back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FetchError {
    /// Cloudflare answered with a challenge, or the API refused (403) for want of a browser's
    /// Turnstile token. Save the answer from a browser and parse that instead.
    Blocked { status: u16 },
    /// Any other HTTP status that is not success.
    Status(u16),
    /// The server took too long.
    Timeout,
    /// No network, DNS, a refused connection, a bad URL, an unreadable body.
    Other(String),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchError::Blocked { status } => write!(
                f,
                "Bazaar DB's bot check refused the request (HTTP {status}); it wants a token only a browser can get"
            ),
            FetchError::Status(status) => write!(f, "Bazaar DB answered HTTP {status}"),
            FetchError::Timeout => write!(f, "Bazaar DB took too long to answer"),
            FetchError::Other(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for FetchError {}

/// How long a fetch may take.
pub const TIMEOUT: Duration = Duration::from_secs(20);

/// The User-Agent: who is asking, not a browser.
pub const USER_AGENT: &str = concat!("BazaarTracker/", env!("CARGO_PKG_VERSION"));

/// Fetches `url` and returns its HTML.
pub fn fetch_page(url: &str) -> Result<String, FetchError> {
    let agent: ureq::Agent =
        ureq::Agent::config_builder().timeout_global(Some(TIMEOUT)).http_status_as_error(false).build().into();
    let mut response = match agent.get(url).header("User-Agent", USER_AGENT).call() {
        Ok(response) => response,
        Err(ureq::Error::Timeout(_)) => return Err(FetchError::Timeout),
        Err(error) => return Err(FetchError::Other(error.to_string())),
    };
    let status = response.status().as_u16();
    let mitigated = response.headers().get("cf-mitigated").is_some();
    let body = response.body_mut().read_to_string().map_err(|e| FetchError::Other(e.to_string()))?;
    classify(status, mitigated, body)
}

/// Decides what an answer means. Split out so it can be tested without a network.
pub(crate) fn classify(status: u16, cf_mitigated: bool, body: String) -> Result<String, FetchError> {
    if cf_mitigated || is_challenge(&body) || status == 403 {
        return Err(FetchError::Blocked { status });
    }
    if !(200..300).contains(&status) {
        return Err(FetchError::Status(status));
    }
    Ok(body)
}

/// Whether `html` is Cloudflare's interstitial rather than the page.
pub fn is_challenge(html: &str) -> bool {
    html.contains("<title>Just a moment...</title>") || html.contains("challenges.cloudflare.com/cdn-cgi")
}

#[cfg(test)]
#[path = "fetch_tests.rs"]
mod tests;
