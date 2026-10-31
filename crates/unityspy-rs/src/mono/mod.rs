//! The Mono runtime's metadata, read through [`crate::mem::reader::Memory`].

pub mod bcl;
pub mod class;
pub mod domain;
pub mod field;
pub mod object;
pub mod offsets;
pub mod pe;
pub mod statics;
pub mod value;
