//! Objects: a header that leads to the runtime class, and fields read on that class.

use std::sync::Arc;

use super::Mono;
use super::class::Class;
use super::value::Value;
use crate::error::{Error, Result};
use crate::mem::reader::{Memory, MemoryExt};

/// The size of a Mono object's header (`vtable*` and `sync`). Field offsets include it, even for
/// a struct that is stored inline without one.
pub const HEADER: u64 = 16;

/// A reference-type object, with the class it really is (not the class its field was declared as).
#[derive(Clone, Debug)]
pub struct Object {
    pub address: u64,
    pub class: Arc<Class>,
}

/// A value type stored inline (in a field, an array or a collection entry), with no object header.
#[derive(Clone, Debug)]
pub struct Struct {
    /// Where the value's first byte is.
    pub address: u64,
    pub class: Arc<Class>,
}

impl<M: Memory> Mono<M> {
    /// The object at `address`: its class comes from `vtable = *object; klass = *vtable`, so fields
    /// resolve on the runtime class and a subclass's fields are reachable.
    pub fn object_at(&self, address: u64) -> Result<Object> {
        let vtable = self.memory.read_ptr(address)?;
        if vtable == 0 {
            return Err(Error::Unexpected(format!("the object at {address:#x} has no vtable (freed, or not an object)")));
        }
        let class = self.memory.read_ptr(vtable)?;
        if class == 0 {
            return Err(Error::Unexpected(format!("the vtable at {vtable:#x} has no class")));
        }
        Ok(Object { address, class: self.class_at(class)? })
    }

    /// The instance field `name` of `object`, searched through its class and parents.
    pub fn read_field(&self, object: &Object, name: &str) -> Result<Value> {
        let field = self.find_field(&object.class, name, None)?;
        if field.is_static() {
            return Err(Error::NotAnInstanceField { class: field.declaring_name, field: field.name });
        }
        self.read_field_slot(&field, object.address + field.offset as u64)
    }

    /// The field `name` of an inline struct. Offsets include the object header the struct does not
    /// have, so 16 is subtracted.
    pub fn read_struct_field(&self, value: &Struct, name: &str) -> Result<Value> {
        let field = self.find_field(&value.class, name, None)?;
        if field.is_static() {
            return Err(Error::NotAnInstanceField { class: field.declaring_name, field: field.name });
        }
        self.read_field_slot(&field, (value.address + field.offset as u64).wrapping_sub(HEADER))
    }
}

#[cfg(test)]
#[path = "object_tests.rs"]
mod tests;
