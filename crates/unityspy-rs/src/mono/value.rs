//! Reading values: primitives at their correct widths, strings, arrays, object references, inline
//! structs and enums. Nothing is converted silently: a value that cannot be read is an error.

use super::Mono;
use super::field::{Field, TypeInfo, type_code};
use super::object::{HEADER, Object, Struct};
use crate::error::{Error, Result};
use crate::mem::reader::{Memory, MemoryExt};
use std::sync::Arc;

use super::class::Class;

/// The longest string we will read, in UTF-16 units. A longer length means a dangling pointer.
pub const MAX_STRING_UNITS: i32 = 4_194_304;
/// The most elements of one array we will accept.
pub const MAX_ARRAY_LENGTH: i32 = 1 << 24;

/// A value read from the game.
#[derive(Clone, Debug)]
pub enum Value {
    /// A null reference.
    Null,
    Bool(bool),
    Char(u16),
    I8(i8),
    U8(u8),
    I16(i16),
    U16(u16),
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
    /// `IntPtr`: 8 bytes on the 64-bit game.
    IntPtr(i64),
    /// `UIntPtr`: 8 bytes on the 64-bit game.
    UIntPtr(u64),
    F32(f32),
    F64(f64),
    Str(String),
    Object(Object),
    Struct(Struct),
    Array(Array),
}

impl Value {
    /// Any integer, `char` or `bool` as an `i64`; `None` for anything else. A `u64` above `i64::MAX` wraps.
    pub fn as_i64(&self) -> Option<i64> {
        Some(match *self {
            Self::Bool(value) => i64::from(value),
            Self::Char(value) => i64::from(value),
            Self::I8(value) => i64::from(value),
            Self::U8(value) => i64::from(value),
            Self::I16(value) => i64::from(value),
            Self::U16(value) => i64::from(value),
            Self::I32(value) => i64::from(value),
            Self::U32(value) => i64::from(value),
            Self::I64(value) | Self::IntPtr(value) => value,
            Self::U64(value) | Self::UIntPtr(value) => value as i64,
            _ => return None,
        })
    }

    pub fn as_bool(&self) -> Option<bool> {
        match *self {
            Self::Bool(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Self::F32(value) => Some(f64::from(value)),
            Self::F64(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&Object> {
        match self {
            Self::Object(object) => Some(object),
            _ => None,
        }
    }

    pub fn as_struct(&self) -> Option<&Struct> {
        match self {
            Self::Struct(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Array> {
        match self {
            Self::Array(array) => Some(array),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

/// An array object. Its elements are read when asked for, not when it is met.
#[derive(Clone, Debug)]
pub struct Array {
    pub address: u64,
    pub length: i32,
    /// The distance between elements (`element_size` of the array class).
    pub stride: i32,
    pub element_class: Arc<Class>,
}

impl Array {
    /// Where the first element is: after the object header, the bounds pointer and the length.
    pub fn data(&self) -> u64 {
        self.address + 32
    }
}

impl<M: Memory> Mono<M> {
    /// The managed string object at `address`: `length` at +16 and UTF-16 characters from +20.
    /// A negative or over-long length is an error, since a dangling pointer reads as garbage.
    pub fn read_string(&self, address: u64) -> Result<String> {
        let length = self.memory.read_i32(address + HEADER)?;
        if !(0..=MAX_STRING_UNITS).contains(&length) {
            return Err(Error::Implausible { what: "string length", value: i64::from(length) });
        }
        let bytes = self.memory.read_bytes(address + HEADER + 4, length as usize * 2)?;
        let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|pair| u16::from_le_bytes(*pair)).collect();
        Ok(String::from_utf16_lossy(&units))
    }

    /// The array object at `address`.
    pub fn array_at(&self, address: u64) -> Result<Array> {
        let array_class = self.object_at(address)?.class;
        let element = self.memory.read_ptr(array_class.address)?;
        if element == 0 {
            return Err(Error::Unexpected(format!("the object at {address:#x} is not an array")));
        }
        let length = self.memory.read_i32(address + 24)?;
        if !(0..=MAX_ARRAY_LENGTH).contains(&length) {
            return Err(Error::Implausible { what: "array length", value: i64::from(length) });
        }
        Ok(Array { address, length, stride: array_class.size, element_class: self.class_at(element)? })
    }

    /// The first `limit` elements of `array` (all of them if it is shorter).
    pub fn array_items(&self, array: &Array, limit: usize) -> Result<Vec<Value>> {
        let count = (array.length as usize).min(limit);
        let element = array.element_class.byval;
        (0..count).map(|i| self.read_value(element, array.data() + i as u64 * array.stride as u64)).collect()
    }

    /// Read the value of type `type_info` that sits at `address`.
    pub fn read_value(&self, type_info: TypeInfo, address: u64) -> Result<Value> {
        let memory = &self.memory;
        Ok(match type_info.type_code() {
            type_code::BOOLEAN => Value::Bool(memory.read_u8(address)? != 0),
            type_code::CHAR => Value::Char(memory.read_u16(address)?),
            // `I1` is signed and `U1` unsigned (UnitySpy had them swapped).
            type_code::I1 => Value::I8(memory.read_i8(address)?),
            type_code::U1 => Value::U8(memory.read_u8(address)?),
            type_code::I2 => Value::I16(memory.read_i16(address)?),
            type_code::U2 => Value::U16(memory.read_u16(address)?),
            type_code::I4 => Value::I32(memory.read_i32(address)?),
            type_code::U4 => Value::U32(memory.read_u32(address)?),
            type_code::I8 => Value::I64(memory.read_i64(address)?),
            type_code::U8 => Value::U64(memory.read_u64(address)?),
            // Native ints are 8 bytes on the 64-bit game (UnitySpy read 4).
            type_code::I => Value::IntPtr(memory.read_i64(address)?),
            type_code::U => Value::UIntPtr(memory.read_u64(address)?),
            // Floats read all of their 4 and 8 bytes (UnitySpy read 2).
            type_code::R4 => Value::F32(memory.read_f32(address)?),
            type_code::R8 => Value::F64(memory.read_f64(address)?),
            type_code::STRING => match memory.read_ptr(address)? {
                0 => Value::Null,
                string => Value::Str(self.read_string(string)?),
            },
            type_code::CLASS | type_code::OBJECT => match memory.read_ptr(address)? {
                0 => Value::Null,
                object => Value::Object(self.object_at(object)?),
            },
            type_code::SZARRAY => match memory.read_ptr(address)? {
                0 => Value::Null,
                array => Value::Array(self.array_at(array)?),
            },
            type_code::GENERICINST => {
                // `data` is a `MonoGenericClass*`, whose first member is the generic definition.
                let container = self.class_at(memory.read_ptr(type_info.data)?)?;
                if container.is_value_type {
                    Value::Struct(Struct { address, class: container })
                } else {
                    match memory.read_ptr(address)? {
                        0 => Value::Null,
                        object => Value::Object(self.object_at(object)?),
                    }
                }
            }
            type_code::VALUETYPE => {
                // A failure to build the struct is an error, never a silent int (UnitySpy's fallback).
                let class = self.class_at(type_info.data)?;
                if class.is_enum {
                    self.read_enum(&class, address)?
                } else {
                    Value::Struct(Struct { address, class })
                }
            }
            other => return Err(Error::UnsupportedType(other)),
        })
    }

    /// The underlying integer of the enum value stored at `address`: its `value__` field.
    fn read_enum(&self, class: &Class, address: u64) -> Result<Value> {
        let field = class.fields.iter().find(|field| field.name == "value__" && !field.is_static()).ok_or_else(|| Error::FieldNotFound { class: class.full_name.clone(), field: "value__".to_owned() })?;
        self.read_value(field.type_info, address.wrapping_add_signed(i64::from(field.offset)).wrapping_sub(HEADER))
    }

    /// Read `field` whose storage starts at `address`. A field whose type is a generic parameter
    /// (a field of a generic definition) cannot be read: the concrete type is not known.
    pub(super) fn read_field_slot(&self, field: &Field, address: u64) -> Result<Value> {
        if field.type_code() == type_code::VAR {
            return Err(Error::UnresolvedGeneric { class: field.declaring_name.clone(), field: field.name.clone() });
        }
        self.read_value(field.type_info, address)
    }
}

#[cfg(test)]
#[path = "value_tests.rs"]
mod tests;
