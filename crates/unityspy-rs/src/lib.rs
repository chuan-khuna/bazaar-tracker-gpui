//! Generic reading of a running Unity (Mono) game's memory, ported from UnitySpy.
//!
//! This crate knows nothing about The Bazaar. It is Windows only: on other platforms it is empty.
//!
//! - [`mem`] attaches to a process and reads raw bytes, through the [`mem::reader::Memory`] trait.
//! - [`mono`] walks the Mono runtime's metadata (domains, classes, fields, statics, objects, values)
//!   using only that trait, so tests can feed it byte buffers instead of a live game.
//!
//! Every module here is a skeleton for now; the memory-reader spike fills them in.
#![cfg(windows)]

pub mod mem;
pub mod mono;
