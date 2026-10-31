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
    ///
    /// Classes are cached, so one cached before the runtime initialised it says it has no vtable
    /// forever; the vtable is then looked for again in the live class.
    pub fn static_data(&self, class: &Class) -> Result<u64> {
        let (vtable, vtable_size) = match class.vtable {
            0 => self.live_vtable(class.address)?,
            vtable => (vtable, class.vtable_size),
        };
        if vtable == 0 {
            return Err(Error::NotInitialised(class.full_name.clone()));
        }
        let slot = vtable + self.offsets.vtable + vtable_size as u64 * 8;
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

    /// The vtable of the class at `address` and its size, as the class is now (0 if it still has none).
    fn live_vtable(&self, address: u64) -> Result<(u64, i32)> {
        let runtime_info = self.memory.read_ptr(address + self.offsets.class_runtime_info)?;
        if runtime_info == 0 {
            return Ok((0, 0));
        }
        let vtable = self.memory.read_ptr(runtime_info + self.offsets.runtime_info_domain_vtables)?;
        Ok((vtable, self.memory.read_i32(address + self.offsets.class_vtable_size)?))
    }
}

#[cfg(test)]
#[path = "statics_tests.rs"]
mod tests;
