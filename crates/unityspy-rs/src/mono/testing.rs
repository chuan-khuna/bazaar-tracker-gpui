//! Builders for fake Mono memory, laid out like the real structures at the Unity 2021/2022 x64
//! offsets, for the memory-seam tests. Only compiled for tests.

use std::cell::Cell;

use super::field::type_code;
use super::offsets::{MonoOffsets, UNITY_2021_2022_X64};
use crate::mem::reader::{BufferMemory, MemoryExt};

pub(crate) const OFFSETS: &MonoOffsets = &UNITY_2021_2022_X64;

/// The field attribute bits that live in the low half of `MonoType.attrs`.
const STATIC: u32 = 0x10;
const LITERAL: u32 = 0x40;

#[derive(Clone)]
pub(crate) struct FieldSpec {
    pub name: &'static str,
    pub code: u8,
    pub offset: i32,
    pub attrs: u32,
    pub type_data: u64,
}

impl FieldSpec {
    pub fn new(name: &'static str, code: u8, offset: i32) -> Self {
        Self { name, code, offset, attrs: 0, type_data: 0 }
    }

    pub fn stat(mut self) -> Self {
        self.attrs |= STATIC;
        self
    }

    pub fn constant(mut self) -> Self {
        self.attrs |= LITERAL | STATIC;
        self
    }

    pub fn data(mut self, data: u64) -> Self {
        self.type_data = data;
        self
    }
}

pub(crate) struct ClassSpec {
    pub name: &'static str,
    pub namespace: &'static str,
    pub parent: u64,
    pub nested_in: u64,
    pub value_type: bool,
    pub enum_type: bool,
    pub kind: u8,
    pub size: i32,
    pub fields: Vec<FieldSpec>,
    pub statics: usize,
    pub initialised: bool,
    pub byval_code: Option<u8>,
    pub element_class: u64,
    pub generic_class: u64,
}

impl ClassSpec {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            namespace: "",
            parent: 0,
            nested_in: 0,
            value_type: false,
            enum_type: false,
            kind: 1,
            size: 0,
            fields: Vec::new(),
            statics: 0,
            initialised: true,
            byval_code: None,
            element_class: 0,
            generic_class: 0,
        }
    }

    pub fn namespace(mut self, namespace: &'static str) -> Self {
        self.namespace = namespace;
        self
    }

    pub fn parent(mut self, parent: u64) -> Self {
        self.parent = parent;
        self
    }

    pub fn nested_in(mut self, outer: u64) -> Self {
        self.nested_in = outer;
        self
    }

    pub fn value_type(mut self) -> Self {
        self.value_type = true;
        self
    }

    pub fn enum_type(mut self) -> Self {
        self.value_type = true;
        self.enum_type = true;
        self
    }

    pub fn field(mut self, field: FieldSpec) -> Self {
        self.fields.push(field);
        self
    }

    pub fn statics(mut self, bytes: usize) -> Self {
        self.statics = bytes;
        self
    }

    pub fn uninitialised(mut self) -> Self {
        self.initialised = false;
        self
    }

    pub fn size(mut self, size: i32) -> Self {
        self.size = size;
        self
    }

    pub fn byval(mut self, code: u8) -> Self {
        self.byval_code = Some(code);
        self
    }
}

/// A class that was built.
#[derive(Clone, Copy)]
pub(crate) struct Built {
    pub address: u64,
    pub vtable: u64,
    pub statics: u64,
}

pub(crate) struct Fake {
    pub memory: BufferMemory,
    next: Cell<u64>,
    counter: Cell<u64>,
}

impl Fake {
    pub fn new() -> Self {
        Self { memory: BufferMemory::new(), next: Cell::new(0x1_0000_0000), counter: Cell::new(0) }
    }

    /// Reserve `size` zeroed bytes at a 16-byte aligned address.
    pub fn alloc(&self, size: u64) -> u64 {
        let address = self.next.get();
        self.next.set(address + size.div_ceil(16) * 16 + 16);
        self.memory.map(address, size.max(1));
        address
    }

    /// A `MonoImage` with a class cache of `buckets` hash buckets.
    pub fn image(&self, buckets: u32) -> u64 {
        let image = self.alloc(OFFSETS.image_class_cache + 0x40);
        let table = self.alloc(u64::from(buckets) * 8);
        let cache = image + OFFSETS.image_class_cache;
        self.memory.write_u32(cache + OFFSETS.hash_table_size, buckets);
        self.memory.write_u64(cache + OFFSETS.hash_table_table, table);
        image
    }

    /// Build a class and add it to the image's class cache.
    pub fn class(&self, image: u64, spec: ClassSpec) -> Built {
        let built = self.class_struct(&spec);
        let cache = image + OFFSETS.image_class_cache;
        let buckets = self.memory.read_u32(cache + OFFSETS.hash_table_size).unwrap();
        let table = self.memory.read_ptr(cache + OFFSETS.hash_table_table).unwrap();
        let bucket = table + (self.counter.get() % u64::from(buckets)) * 8;
        self.counter.set(self.counter.get() + 1);
        let head = self.memory.read_ptr(bucket).unwrap();
        self.memory.write_u64(built.address + OFFSETS.class_next_class_cache, head);
        self.memory.write_u64(bucket, built.address);
        built
    }

    /// Build a class without adding it to any class cache (array and generic instance classes).
    pub fn class_struct(&self, spec: &ClassSpec) -> Built {
        let class = self.alloc(0x120);
        let m = &self.memory;
        self.c_string_at(class + OFFSETS.class_name, spec.name);
        self.c_string_at(class + OFFSETS.class_namespace, spec.namespace);
        m.write_u64(class + OFFSETS.class_parent, spec.parent);
        m.write_u64(class + OFFSETS.class_nested_in, spec.nested_in);
        m.write_u32(class + OFFSETS.class_bit_fields, (u32::from(spec.value_type) * 4) | (u32::from(spec.enum_type) * 8));
        m.write_u8(class + OFFSETS.class_kind, spec.kind);
        m.write_i32(class + OFFSETS.class_size, spec.size);
        m.write_i32(class + OFFSETS.class_field_count, spec.fields.len() as i32);
        m.write_u64(class + OFFSETS.class_generic_class, spec.generic_class);
        // `element_class` is the first member of a `MonoClass`.
        m.write_u64(class, spec.element_class);
        let code = spec.byval_code.unwrap_or(if spec.value_type { type_code::VALUETYPE } else { type_code::CLASS });
        m.write_u64(class + OFFSETS.class_byval_arg, class);
        m.write_u32(class + OFFSETS.class_byval_arg + 8, u32::from(code) << 16);

        if !spec.fields.is_empty() {
            let fields = self.alloc(spec.fields.len() as u64 * OFFSETS.class_field_size);
            m.write_u64(class + OFFSETS.class_fields, fields);
            for (i, field) in spec.fields.iter().enumerate() {
                let entry = fields + i as u64 * OFFSETS.class_field_size;
                let type_ptr = self.alloc(16);
                m.write_u64(type_ptr, field.type_data);
                m.write_u32(type_ptr + 8, u32::from(field.code) << 16 | field.attrs);
                m.write_u64(entry, type_ptr);
                self.c_string_at(entry + 8, field.name);
                m.write_i32(entry + 24, field.offset);
            }
        }

        let (mut vtable, mut statics) = (0, 0);
        if spec.initialised {
            let vtable_size = 4u64;
            let runtime_info = self.alloc(32);
            vtable = self.alloc(OFFSETS.vtable + vtable_size * 8 + 16);
            m.write_u64(runtime_info + OFFSETS.runtime_info_domain_vtables, vtable);
            m.write_u64(class + OFFSETS.class_runtime_info, runtime_info);
            m.write_u32(class + OFFSETS.class_vtable_size, vtable_size as u32);
            m.write_u64(vtable, class);
            if spec.statics > 0 {
                statics = self.alloc(spec.statics as u64);
                m.write_u64(vtable + OFFSETS.vtable + vtable_size * 8, statics);
            }
        }
        Built { address: class, vtable, statics }
    }

    fn c_string_at(&self, pointer_slot: u64, text: &str) {
        let text_at = self.alloc(text.len() as u64 + 1);
        self.memory.write_c_string(text_at, text);
        self.memory.write_u64(pointer_slot, text_at);
    }

    /// An object of `class`: a header whose vtable leads back to the class, then `body` bytes.
    pub fn object(&self, class: Built, body: usize) -> u64 {
        assert!(class.vtable != 0, "class has no vtable");
        let object = self.alloc(16 + body as u64);
        self.memory.write_u64(object, class.vtable);
        object
    }

    /// A `System.String` object holding `text`.
    pub fn string(&self, text: &str) -> u64 {
        let units: Vec<u16> = text.encode_utf16().collect();
        let string = self.alloc(20 + units.len() as u64 * 2 + 2);
        self.memory.write_i32(string + 16, units.len() as i32);
        let bytes: Vec<u8> = units.iter().flat_map(|unit| unit.to_le_bytes()).collect();
        self.memory.write(string + 20, &bytes);
        string
    }

    /// A string object with a chosen (possibly bogus) length field and no characters.
    pub fn string_with_length(&self, length: i32) -> u64 {
        let string = self.alloc(64);
        self.memory.write_i32(string + 16, length);
        string
    }

    /// An array class for elements of `element` laid out `element_size` bytes apart.
    pub fn array_class(&self, element: Built, element_size: i32) -> Built {
        let mut spec = ClassSpec::new("Array").size(element_size);
        spec.kind = 5;
        spec.element_class = element.address;
        self.class_struct(&spec)
    }

    /// An array object of `class` holding `count` elements whose bytes are `data`.
    pub fn array(&self, class: Built, count: i32, data: &[u8]) -> u64 {
        let array = self.alloc(32 + data.len() as u64);
        self.memory.write_u64(array, class.vtable);
        self.memory.write_i32(array + 24, count);
        self.memory.write(array + 32, data);
        array
    }
}
