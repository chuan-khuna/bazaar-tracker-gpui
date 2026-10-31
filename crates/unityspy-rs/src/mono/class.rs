//! Mono classes: lazy lookup in an image's class cache by name and namespace, cached by full name.
//!
//! Unlike UnitySpy, nothing is read for classes we are not asked about: the cache is walked only
//! until the wanted class is found, and the class model is read in one call.

use std::sync::Arc;

use super::Mono;
use super::field::{self, Field, TypeInfo};
use crate::error::{Error, Result};
use crate::mem::reader::{Memory, MemoryExt};

/// `MonoTypeKind`: what sort of class this is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassKind {
    /// An ordinary class or struct.
    Def,
    /// A generic type definition (`List<T>`).
    GenericDefinition,
    /// A generic instance (`List<int>`).
    GenericInstance,
    GenericParameter,
    Array,
    Pointer,
    Unknown(u8),
}

impl ClassKind {
    fn from_byte(byte: u8) -> Self {
        match byte & 7 {
            1 => Self::Def,
            2 => Self::GenericDefinition,
            3 => Self::GenericInstance,
            4 => Self::GenericParameter,
            5 => Self::Array,
            6 => Self::Pointer,
            other => Self::Unknown(other),
        }
    }
}

/// A `MonoClass`, read in one call, with the fields it declares itself.
#[derive(Clone, Debug)]
pub struct Class {
    pub address: u64,
    pub name: String,
    pub namespace: String,
    /// `Namespace.Name`, or `Outer.Namespace.Outer+Inner` for a nested class.
    pub full_name: String,
    /// The parent class's address, or 0.
    pub parent: u64,
    /// The declaring class's address, or 0 if this class is not nested.
    pub nested_in: u64,
    /// The raw flag word.
    pub flags: u32,
    pub is_value_type: bool,
    pub is_enum: bool,
    /// `sizes`: the instance size, or for an array class the element size.
    pub size: i32,
    pub field_count: i32,
    /// The class's vtable in the root domain, or 0 if the runtime has not initialised it.
    pub vtable: u64,
    pub vtable_size: i32,
    pub kind: ClassKind,
    /// The class seen as a type (`_byval_arg`): an array element is read by this.
    pub byval: TypeInfo,
    /// The `MonoGenericClass*` of a generic instance, or 0.
    pub generic_class: u64,
    /// The fields this class declares, in declaration order. Parents' fields are not included.
    pub fields: Vec<Field>,
}

const FLAG_VALUE_TYPE: u32 = 0x4;
const FLAG_ENUM: u32 = 0x8;
/// The most classes in one hash bucket, or in one image, before the cache is called corrupt.
const MAX_CHAIN: usize = 1_000_000;
const MAX_BUCKETS: u32 = 1 << 24;
/// How many levels of nesting we will follow.
const MAX_NESTING: usize = 16;

fn le32(block: &[u8], at: u64) -> u32 {
    let at = at as usize;
    u32::from_le_bytes(block[at..at + 4].try_into().unwrap())
}

fn le64(block: &[u8], at: u64) -> u64 {
    let at = at as usize;
    u64::from_le_bytes(block[at..at + 8].try_into().unwrap())
}

impl<M: Memory> Mono<M> {
    /// The class at this `MonoClass*`, read once and cached.
    pub fn class_at(&self, address: u64) -> Result<Arc<Class>> {
        self.class_at_depth(address, 0)
    }

    fn class_at_depth(&self, address: u64, depth: usize) -> Result<Arc<Class>> {
        if let Some(class) = self.caches.lock().unwrap().by_address.get(&address) {
            return Ok(class.clone());
        }
        let class = Arc::new(self.read_class(address, depth)?);
        self.caches.lock().unwrap().by_address.insert(address, class.clone());
        Ok(class)
    }

    fn read_class(&self, address: u64, depth: usize) -> Result<Class> {
        if depth > MAX_NESTING {
            return Err(Error::Loop("nesting chain"));
        }
        let offsets = self.offsets;
        // One read for the whole struct; every member is decoded from this block.
        let block = self.memory.read_bytes(address, offsets.class_next_class_cache as usize + 8)?;
        let flags = le32(&block, offsets.class_bit_fields);
        let kind = ClassKind::from_byte(block[offsets.class_kind as usize]);
        let name = self.memory.read_c_string(le64(&block, offsets.class_name))?;
        let namespace = match le64(&block, offsets.class_namespace) {
            0 => String::new(),
            ptr => self.memory.read_c_string(ptr)?,
        };
        let parent = le64(&block, offsets.class_parent);
        let nested_in = le64(&block, offsets.class_nested_in);
        let generic_class = le64(&block, offsets.class_generic_class);
        let runtime_info = le64(&block, offsets.class_runtime_info);
        let vtable = if runtime_info == 0 { 0 } else { self.memory.read_ptr(runtime_info + offsets.runtime_info_domain_vtables)? };
        let vtable_size = if runtime_info == 0 { 0 } else { le32(&block, offsets.class_vtable_size) as i32 };

        let full_name = if nested_in != 0 {
            format!("{}+{name}", self.class_at_depth(nested_in, depth + 1)?.full_name)
        } else if namespace.is_empty() {
            name.clone()
        } else {
            format!("{namespace}.{name}")
        };

        // A generic instance has no field count of its own: it is the definition's.
        let (mut field_count, mut fields_ptr) = (le32(&block, offsets.class_field_count) as i32, le64(&block, offsets.class_fields));
        match kind {
            ClassKind::GenericInstance if generic_class != 0 => {
                let container = self.memory.read_ptr(generic_class)?;
                field_count = self.memory.read_i32(container + offsets.class_field_count)?;
                if fields_ptr == 0 {
                    fields_ptr = self.memory.read_ptr(container + offsets.class_fields)?;
                }
            }
            ClassKind::Def | ClassKind::GenericDefinition => {}
            _ => field_count = 0,
        }
        let fields = field::read_fields(self, fields_ptr, field_count, address, &full_name)?;

        Ok(Class {
            address,
            name,
            namespace,
            full_name,
            parent,
            nested_in,
            flags,
            is_value_type: flags & FLAG_VALUE_TYPE != 0,
            is_enum: flags & FLAG_ENUM != 0,
            size: le32(&block, offsets.class_size) as i32,
            field_count,
            vtable,
            vtable_size,
            kind,
            byval: TypeInfo { data: le64(&block, offsets.class_byval_arg), attrs: le32(&block, offsets.class_byval_arg + 8) },
            generic_class,
            fields,
        })
    }

    /// Visit the address of every class in the image's class cache until `visit` returns `true`.
    fn walk_class_cache(&self, image: u64, mut visit: impl FnMut(u64) -> Result<bool>) -> Result<()> {
        let offsets = self.offsets;
        let cache = image + offsets.image_class_cache;
        let size = self.memory.read_u32(cache + offsets.hash_table_size)?;
        let table = self.memory.read_ptr(cache + offsets.hash_table_table)?;
        if size > MAX_BUCKETS {
            return Err(Error::Implausible { what: "class cache size", value: i64::from(size) });
        }
        let mut seen = 0usize;
        for bucket in 0..u64::from(size) {
            let mut class = self.memory.read_ptr(table + bucket * 8)?;
            while class != 0 {
                seen += 1;
                if seen > MAX_CHAIN {
                    return Err(Error::Loop("class cache"));
                }
                if visit(class)? {
                    return Ok(());
                }
                class = self.memory.read_ptr(class + offsets.class_next_class_cache)?;
            }
        }
        Ok(())
    }

    /// The top-level class `name` in `namespace` (empty for the global namespace) of `image`.
    fn find_top_level(&self, image: u64, namespace: &str, name: &str) -> Result<Option<u64>> {
        let offsets = self.offsets;
        let mut found = None;
        self.walk_class_cache(image, |class| {
            if self.memory.read_ptr(class + offsets.class_nested_in)? != 0 {
                return Ok(false);
            }
            if self.memory.read_c_string(self.memory.read_ptr(class + offsets.class_name)?)? != name {
                return Ok(false);
            }
            let namespace_ptr = self.memory.read_ptr(class + offsets.class_namespace)?;
            let class_namespace = if namespace_ptr == 0 { String::new() } else { self.memory.read_c_string(namespace_ptr)? };
            if class_namespace == namespace {
                found = Some(class);
            }
            Ok(found.is_some())
        })?;
        Ok(found)
    }

    /// The class nested in `outer` called `name`.
    fn find_nested(&self, image: u64, outer: u64, name: &str) -> Result<Option<u64>> {
        let offsets = self.offsets;
        let mut found = None;
        self.walk_class_cache(image, |class| {
            if self.memory.read_ptr(class + offsets.class_nested_in)? != outer {
                return Ok(false);
            }
            if self.memory.read_c_string(self.memory.read_ptr(class + offsets.class_name)?)? == name {
                found = Some(class);
            }
            Ok(found.is_some())
        })?;
        Ok(found)
    }

    /// The class with this full name in `image`: `Namespace.Name`, `Name` for the global
    /// namespace, or `Outer+Inner` for a nested class. The class cache is walked only as far as
    /// needed, and the answer is remembered. A missing class is an error naming it.
    pub fn find_class(&self, image: u64, full_name: &str) -> Result<Arc<Class>> {
        let key = (image, full_name.to_owned());
        let cached = self.caches.lock().unwrap().by_name.get(&key).copied();
        if let Some(address) = cached {
            return self.class_at(address);
        }
        let not_found = || Error::ClassNotFound(full_name.to_owned());
        let mut parts = full_name.split('+');
        let first = parts.next().unwrap_or_default();
        let (namespace, name) = first.rsplit_once('.').unwrap_or(("", first));
        let mut address = self.find_top_level(image, namespace, name)?.ok_or_else(not_found)?;
        for nested in parts {
            address = self.find_nested(image, address, nested)?.ok_or_else(not_found)?;
        }
        self.caches.lock().unwrap().by_name.insert(key, address);
        self.class_at(address)
    }
}

#[cfg(test)]
#[path = "class_tests.rs"]
mod tests;
