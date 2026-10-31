//! Static fields, read through the class's vtable.

use super::Mono;
use super::class::Class;
use super::value::Value;
use crate::error::{Error, Result};
use crate::mem::reader::{Memory, MemoryExt};

impl<M: Memory> Mono<M> {
    /// The address of the block that holds `class`'s static fields: the slot just after the vtable's
    /// method pointers. A class the runtime has not initialised has no vtable, which is its own
    /// error ([`Error::NotInitialised`]) rather than a read at a small absolute address.
    pub fn static_data(&self, class: &Class) -> Result<u64> {
        if class.vtable == 0 {
            return Err(Error::NotInitialised(class.full_name.clone()));
        }
        let slot = class.vtable + self.offsets.vtable + class.vtable_size as u64 * 8;
        match self.memory.read_ptr(slot)? {
            0 => Err(Error::NotInitialised(class.full_name.clone())),
            data => Ok(data),
        }
    }

    /// The value of the static field `name` of `class`. A field declared by a parent class is read
    /// from the parent's static data (which is how a singleton's `Instance` is reached through its
    /// base class). A `const` has no storage and cannot be read.
    pub fn read_static(&self, class: &Class, name: &str) -> Result<Value> {
        let field = self.find_field(class, name, None)?;
        if !field.is_static() || field.is_constant() {
            return Err(Error::NotAStaticField { class: field.declaring_name, field: field.name });
        }
        let declaring = self.class_at(field.declaring_class)?;
        let data = self.static_data(&declaring)?;
        self.read_field_slot(&field, data.wrapping_add_signed(i64::from(field.offset)))
    }
}

#[cfg(test)]
#[path = "statics_tests.rs"]
mod tests;
