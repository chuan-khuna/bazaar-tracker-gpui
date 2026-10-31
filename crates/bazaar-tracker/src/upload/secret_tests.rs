use super::*;

#[test]
fn release_and_debug_builds_use_different_entries() {
    assert_eq!(service_name(false), "BazaarTracker");
    assert_eq!(service_name(true), "bazaar-tracker-dev");
    assert_ne!(service_name(true), service_name(false));
}

#[test]
fn this_build_uses_the_entry_for_its_kind() {
    let expected = if cfg!(debug_assertions) { "bazaar-tracker-dev" } else { "BazaarTracker" };
    assert_eq!(SecretStore::new().service(), expected);
}

#[test]
fn an_empty_key_is_not_saved() {
    let store = SecretStore::with_service(&format!("bazaar-tracker-test-empty-{}", std::process::id()));
    assert!(store.save("").is_err());
    assert!(store.save("   ").is_err());
    assert_eq!(store.load().unwrap(), None);
}

/// This talks to the real Windows Credential Manager, under an entry made for the test and removed after.
#[cfg(windows)]
#[test]
fn a_key_is_saved_loaded_replaced_and_cleared_in_the_credential_store() {
    struct Cleanup(SecretStore);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = self.0.clear();
        }
    }
    let store = SecretStore::with_service(&format!("bazaar-tracker-test-{}", std::process::id()));
    let _cleanup = Cleanup(store.clone());

    assert_eq!(store.load().unwrap(), None, "nothing saved yet");
    store.save("first-key").unwrap();
    assert_eq!(store.load().unwrap().as_deref(), Some("first-key"));
    store.save("second-key").unwrap();
    assert_eq!(store.load().unwrap().as_deref(), Some("second-key"));
    store.clear().unwrap();
    assert_eq!(store.load().unwrap(), None);
    // Clearing what is not there is not an error.
    store.clear().unwrap();
}
