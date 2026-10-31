//! Mono class fields: name, type code, attributes, offset and type data. Looking a field up searches
//! the parent chain and can be limited to one declaring type.

use std::sync::Arc;

use super::Mono;
use super::class::Class;
use crate::error::{Error, Result};
use crate::mem::reader::{Memory, MemoryExt};

/// `MonoTypeEnum`: the kind of a field's type.
pub mod type_code {
    pub const BOOLEAN: u8 = 0x02;
    pub const CHAR: u8 = 0x03;
    pub const I1: u8 = 0x04;
    pub const U1: u8 = 0x05;
    pub const I2: u8 = 0x06;
    pub const U2: u8 = 0x07;
    pub const I4: u8 = 0x08;
    pub const U4: u8 = 0x09;
    pub const I8: u8 = 0x0A;
    pub const U8: u8 = 0x0B;
    pub const R4: u8 = 0x0C;
    pub const R8: u8 = 0x0D;
    pub const STRING: u8 = 0x0E;
    pub const PTR: u8 = 0x0F;
    pub const VALUETYPE: u8 = 0x11;
    pub const CLASS: u8 = 0x12;
    pub const VAR: u8 = 0x13;
    pub const ARRAY: u8 = 0x14;
    pub const GENERICINST: u8 = 0x15;
    pub const I: u8 = 0x18;
    pub const U: u8 = 0x19;
    pub const OBJECT: u8 = 0x1C;
    pub const SZARRAY: u8 = 0x1D;
}

/// The field is static (`FIELD_ATTRIBUTE_STATIC`), in the low bits of `MonoType.attrs`.
const ATTR_STATIC: u32 = 0x10;
/// The field is a literal (`const`): it has no storage.
const ATTR_LITERAL: u32 = 0x40;

/// A `MonoType`: what a field holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypeInfo {
    /// The `data` union: a `MonoClass*`, a `MonoGenericClass*`, a generic parameter, or an array.
    pub data: u64,
    /// The raw `attrs` word: field attributes in the low 16 bits, the type code in bits 16-23.
    pub attrs: u32,
}

impl TypeInfo {
    pub fn type_code(&self) -> u8 {
        (self.attrs >> 16) as u8
    }

    pub fn is_static(&self) -> bool {
        self.attrs & ATTR_STATIC != 0
    }

    pub fn is_constant(&self) -> bool {
        self.attrs & ATTR_LITERAL != 0
    }
}

/// A field of a class, as `MonoClassField` describes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub type_info: TypeInfo,
    /// Offset in the instance (including the 16-byte object header, even for structs) or, for a
    /// static, in the class's static data.
    pub offset: i32,
    /// The address of the class that declares this field.
    pub declaring_class: u64,
    /// The full name of the class that declares this field.
    pub declaring_name: String,
}

impl Field {
    pub fn type_code(&self) -> u8 {
        self.type_info.type_code()
    }

    pub fn is_static(&self) -> bool {
        self.type_info.is_static()
    }

    pub fn is_constant(&self) -> bool {
        self.type_info.is_constant()
    }

    pub fn attrs(&self) -> u32 {
        self.type_info.attrs
    }

    pub fn type_data(&self) -> u64 {
        self.type_info.data
    }
}

/// The most fields we will read from one class before deciding the count is corrupt.
const MAX_FIELDS: i32 = 8192;
/// How many parents we will climb before deciding the chain is corrupt.
const MAX_DEPTH: usize = 64;

/// Read the `count` fields of the array at `fields`, for the class `declaring`.
pub(super) fn read_fields<M: Memory>(mono: &Mono<M>, fields: u64, count: i32, declaring: u64, declaring_name: &str) -> Result<Vec<Field>> {
    if !(0..=MAX_FIELDS).contains(&count) {
        return Err(Error::Implausible { what: "field count", value: i64::from(count) });
    }
    if fields == 0 || count == 0 {
        return Ok(Vec::new());
    }
    let stride = mono.offsets.class_field_size as usize;
    let block = mono.memory.read_bytes(fields, stride * count as usize)?;
    let mut found = Vec::with_capacity(count as usize);
    for entry in block.chunks_exact(stride) {
        let type_ptr = u64::from_le_bytes(entry[0..8].try_into().unwrap());
        if type_ptr == 0 {
            break;
        }
        let name_ptr = u64::from_le_bytes(entry[8..16].try_into().unwrap());
        let offset = i32::from_le_bytes(entry[24..28].try_into().unwrap());
        let type_info = TypeInfo { data: mono.memory.read_ptr(type_ptr)?, attrs: mono.memory.read_u32(type_ptr + 8)? };
        found.push(Field { name: mono.memory.read_c_string(name_ptr)?, type_info, offset, declaring_class: declaring, declaring_name: declaring_name.to_owned() });
    }
    Ok(found)
}

impl<M: Memory> Mono<M> {
    /// The field `name` of `class` or its parents, nearest first. With `declaring_type` (a full
    /// class name) only a field declared by that class counts, which reaches a field that a
    /// subclass hides.
    pub fn find_field(&self, class: &Class, name: &str, declaring_type: Option<&str>) -> Result<Field> {
        let mut current = self.class_at(class.address)?;
        for _ in 0..MAX_DEPTH {
            if declaring_type.is_none_or(|wanted| wanted == current.full_name) {
                if let Some(field) = current.fields.iter().find(|field| field.name == name) {
                    return Ok(field.clone());
                }
            }
            if current.parent == 0 {
                break;
            }
            current = self.class_at(current.parent)?;
        }
        Err(Error::FieldNotFound { class: class.full_name.clone(), field: name.to_owned() })
    }

    /// Every field of `class`, then of its parent, and so on up the chain.
    pub fn fields_with_inherited(&self, class: &Class) -> Result<Vec<Field>> {
        let mut all = Vec::new();
        let mut current: Arc<Class> = self.class_at(class.address)?;
        for _ in 0..MAX_DEPTH {
            all.extend(current.fields.iter().cloned());
            if current.parent == 0 {
                return Ok(all);
            }
            current = self.class_at(current.parent)?;
        }
        Err(Error::Loop("parent chain"))
    }
}
