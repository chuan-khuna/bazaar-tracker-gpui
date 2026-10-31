//! The secret key, kept in the operating system's credential store (the Windows Credential
//! Manager), never in a plain file. A debug build uses a different entry from a release build, so
//! development never overwrites the real key.

use std::fmt;

/// The credential entry release builds use.
pub const RELEASE_SERVICE: &str = "BazaarTracker";
/// The credential entry debug builds use.
pub const DEBUG_SERVICE: &str = "bazaar-tracker-dev";
/// The user name of the entry (the credential store wants one).
const ENTRY_USER: &str = "secret-key";

/// The entry name for a debug or a release build.
pub fn service_name(debug_build: bool) -> &'static str {
    if debug_build { DEBUG_SERVICE } else { RELEASE_SERVICE }
}

#[derive(Debug)]
pub struct SecretError(String);

impl fmt::Display for SecretError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "credential store: {}", self.0)
    }
}

impl std::error::Error for SecretError {}

impl From<keyring::Error> for SecretError {
    fn from(error: keyring::Error) -> Self {
        Self(error.to_string())
    }
}

/// The secret key in the credential store.
#[derive(Clone, Debug)]
pub struct SecretStore {
    service: String,
}

impl SecretStore {
    /// The store for this kind of build: `BazaarTracker` in release, `bazaar-tracker-dev` in debug.
    pub fn new() -> Self {
        Self::with_service(service_name(cfg!(debug_assertions)))
    }

    /// A store under another entry name, for tests.
    pub fn with_service(service: &str) -> Self {
        Self { service: service.to_owned() }
    }

    pub fn service(&self) -> &str {
        &self.service
    }

    fn entry(&self) -> Result<keyring::Entry, SecretError> {
        Ok(keyring::Entry::new(&self.service, ENTRY_USER)?)
    }

    /// The saved key, if there is one.
    pub fn load(&self) -> Result<Option<String>, SecretError> {
        match self.entry()?.get_password() {
            Ok(key) => Ok(Some(key)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Save the key, replacing any saved one. An empty key is not saved.
    pub fn save(&self, key: &str) -> Result<(), SecretError> {
        if key.trim().is_empty() {
            return Err(SecretError("a key cannot be empty".to_owned()));
        }
        Ok(self.entry()?.set_password(key)?)
    }

    /// Forget the key.
    pub fn clear(&self) -> Result<(), SecretError> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

impl Default for SecretStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "secret_tests.rs"]
mod tests;
