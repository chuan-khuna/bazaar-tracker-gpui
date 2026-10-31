//! Uploading Runs to Bazaar DB (off by default), and the secret key in the Windows credential store.
//!
//! - [`uploader`] picks sendable Events, sends them in batches and backs off on failure.
//! - [`batch`] decides what goes in a batch and writes it.
//! - [`transport`] is the HTTP side, behind a trait.
//! - [`secret`] is the secret key.

pub mod batch;
pub mod secret;
pub mod transport;
pub mod uploader;

pub use secret::SecretStore;
pub use uploader::{Drain, UploadConfig, UploadHandle, UploadNotice, Uploader};
