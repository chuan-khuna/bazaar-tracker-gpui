//! Unity serialized files (the `CAB-<hash>` inside a bundle): the type table with each type's
//! type tree, the object table, and reading an object into a [`Value`] by its type tree. The
//! game's bundles keep their type trees, so no class layouts are built in here.

use std::collections::HashMap;

use super::common_strings::COMMON_STRINGS;
use super::{Reader, Result, error};

/// A field's meta flag bit: align the stream to 4 bytes after reading it.
const ALIGN: u32 = 0x4000;

/// An object read by its type tree.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    Str(String),
    /// A byte array (`vector<UInt8>`, `TypelessData`).
    Bytes(Vec<u8>),
    List(Vec<Value>),
    Pair(Box<Value>, Box<Value>),
    /// A class: its fields in order.
    Object(Vec<(String, Value)>),
}

impl Value {
    /// The field called `name`, if this is a class that has one.
    pub fn get(&self, name: &str) -> Option<&Value> {
        match self {
            Value::Object(fields) => fields.iter().find(|(field, _)| field == name).map(|(_, value)| value),
            _ => None,
        }
    }

    /// The field at the end of a path of field names.
    pub fn at(&self, path: &[&str]) -> Option<&Value> {
        path.iter().try_fold(self, |value, name| value.get(name))
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(value) => Some(*value),
            Value::UInt(value) => i64::try_from(*value).ok(),
            Value::Bool(value) => Some(i64::from(*value)),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Value::Bytes(bytes) => Some(bytes),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(items) => Some(items),
            _ => None,
        }
    }

    /// A `PPtr`'s (file id, path id). File id 0 is this file; path id 0 is no object.
    pub fn as_pptr(&self) -> Option<(i32, i64)> {
        Some((self.get("m_FileID")?.as_i64()? as i32, self.get("m_PathID")?.as_i64()?))
    }
}

/// A node of a type tree, with its children worked out from the levels.
#[derive(Debug)]
struct Node {
    type_name: String,
    name: String,
    meta_flag: u32,
    children: Vec<usize>,
}

impl Node {
    fn aligned(&self) -> bool {
        self.meta_flag & ALIGN != 0
    }
}

#[derive(Debug)]
struct Type {
    class_id: i32,
    /// Empty when the file has no type tree for it.
    nodes: Vec<Node>,
}

/// One object in the file.
#[derive(Clone, Copy, Debug)]
pub struct ObjectInfo {
    pub path_id: i64,
    start: usize,
    size: usize,
    type_index: usize,
}

pub struct SerializedFile<'a> {
    data: &'a [u8],
    big_endian: bool,
    types: Vec<Type>,
    objects: HashMap<i64, ObjectInfo>,
}

/// Unity class ids used here.
pub mod class {
    pub const TEXTURE_2D: i32 = 28;
    pub const MATERIAL: i32 = 21;
    pub const MONO_BEHAVIOUR: i32 = 114;
    pub const ASSET_BUNDLE: i32 = 142;
    pub const SPRITE: i32 = 213;
}

impl<'a> SerializedFile<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        let mut header = Reader::new(data, true);
        let _metadata_size = header.u32()?;
        let _file_size = header.u32()?;
        let version = header.u32()?;
        let mut data_offset = header.u32()? as usize;
        if version < 9 {
            return error(format!("serialized file version {version} is too old"));
        }
        let big_endian = header.u8()? != 0;
        header.bytes(3)?;
        if version >= 22 {
            let _metadata_size = header.u32()?;
            let _file_size = header.i64()?;
            data_offset = header.i64()? as usize;
            header.i64()?;
        }
        let mut reader = Reader { pos: header.pos, ..Reader::new(data, big_endian) };
        let _unity_version = reader.cstring()?;
        let _platform = reader.i32()?;
        let type_trees = reader.u8()? != 0;
        let types = (0..reader.count()?).map(|_| read_type(&mut reader, version, type_trees, false)).collect::<Result<Vec<_>>>()?;
        if version < 14 {
            reader.i32()?; // big ids enabled
        }
        let mut objects = HashMap::new();
        for _ in 0..reader.count()? {
            if version >= 14 {
                reader.align(4);
            }
            let path_id = if version >= 14 { reader.i64()? } else { i64::from(reader.i32()?) };
            let start = if version >= 22 { reader.i64()? as usize } else { reader.u32()? as usize } + data_offset;
            let size = reader.u32()? as usize;
            let type_index = reader.i32()?;
            if version < 16 {
                return error(format!("serialized file version {version} is not supported"));
            }
            let type_index = usize::try_from(type_index).ok().filter(|index| *index < types.len()).ok_or_else(|| super::FormatError(format!("object {path_id} has no type")))?;
            if start + size > data.len() {
                return error(format!("object {path_id} lies outside the file"));
            }
            objects.insert(path_id, ObjectInfo { path_id, start, size, type_index });
        }
        Ok(Self { data, big_endian, types, objects })
    }

    pub fn objects(&self) -> impl Iterator<Item = &ObjectInfo> {
        self.objects.values()
    }

    pub fn object(&self, path_id: i64) -> Option<&ObjectInfo> {
        self.objects.get(&path_id)
    }

    pub fn class_id(&self, object: &ObjectInfo) -> i32 {
        self.types[object.type_index].class_id
    }

    /// Read an object by its type tree.
    pub fn read(&self, object: &ObjectInfo) -> Result<Value> {
        let nodes = &self.types[object.type_index].nodes;
        if nodes.is_empty() {
            return error(format!("object {} has no type tree", object.path_id));
        }
        let mut reader = Reader::new(&self.data[object.start..object.start + object.size], self.big_endian);
        read_value(nodes, 0, &mut reader, true)
    }
}

fn read_type(reader: &mut Reader, version: u32, type_trees: bool, is_ref_type: bool) -> Result<Type> {
    let class_id = reader.i32()?;
    if version >= 16 {
        reader.u8()?; // stripped
    }
    if version >= 17 {
        reader.i16()?; // script type index
    }
    if version >= 13 {
        if is_ref_type || class_id == class::MONO_BEHAVIOUR {
            reader.bytes(16)?; // script id
        }
        reader.bytes(16)?; // old type hash
    }
    let mut nodes = Vec::new();
    if type_trees {
        let mut has_tree = true;
        if version >= 23 {
            reader.bytes(16)?; // content hash
            has_tree = reader.i32()? != 0;
        }
        if has_tree {
            nodes = read_type_tree(reader, version)?;
        }
        if version >= 21 {
            if is_ref_type {
                reader.cstring()?;
                reader.cstring()?;
                reader.cstring()?;
            } else {
                let count = reader.count()?;
                reader.bytes(count * 4)?; // type dependencies
            }
        }
    }
    Ok(Type { class_id, nodes })
}

/// A type tree "blob": the nodes as fixed records, then their strings.
fn read_type_tree(reader: &mut Reader, version: u32) -> Result<Vec<Node>> {
    if version >= 23 {
        if reader.bytes(4)? != b"mhtt" {
            return error("bad type tree magic");
        }
        reader.i32()?;
    }
    let count = reader.count()?;
    let strings_size = reader.count()?;
    let mut raw = Vec::with_capacity(count);
    for _ in 0..count {
        let _version = reader.u16()?;
        let level = reader.u8()?;
        let _type_flags = reader.u8()?;
        let type_offset = reader.u32()?;
        let name_offset = reader.u32()?;
        let _byte_size = reader.i32()?;
        let _index = reader.i32()?;
        let meta_flag = reader.u32()?;
        if version >= 19 {
            reader.u64()?; // ref type hash
        }
        raw.push((level, type_offset, name_offset, meta_flag));
    }
    let strings = reader.bytes(strings_size)?;
    let mut nodes: Vec<Node> = raw.iter().map(|(_, type_offset, name_offset, meta_flag)| Node { type_name: tree_string(strings, *type_offset), name: tree_string(strings, *name_offset), meta_flag: *meta_flag, children: Vec::new() }).collect();
    // A node's parent is the nearest node before it one level up.
    let mut stack: Vec<usize> = Vec::new();
    for (index, (level, ..)) in raw.iter().enumerate() {
        stack.truncate(usize::from(*level));
        if let Some(parent) = stack.last() {
            nodes[*parent].children.push(index);
        }
        stack.push(index);
    }
    Ok(nodes)
}

/// A string of a type tree: in the tree's own buffer, or Unity's common strings when the top bit is set.
fn tree_string(local: &[u8], offset: u32) -> String {
    let (buffer, start) = if offset & 0x8000_0000 == 0 { (local, offset as usize) } else { (COMMON_STRINGS.as_bytes(), (offset & 0x7fff_ffff) as usize) };
    let rest = buffer.get(start..).unwrap_or_default();
    let end = rest.iter().position(|byte| *byte == 0).unwrap_or(rest.len());
    String::from_utf8_lossy(&rest[..end]).into_owned()
}

/// A primitive of the given type name, or None if it is not one.
fn read_primitive(type_name: &str, reader: &mut Reader) -> Option<Result<Value>> {
    let value = match type_name {
        "SInt8" => reader.u8().map(|v| Value::Int(i64::from(v as i8))),
        "UInt8" | "char" => reader.u8().map(|v| Value::UInt(u64::from(v))),
        "bool" => reader.u8().map(|v| Value::Bool(v != 0)),
        "short" | "SInt16" => reader.i16().map(|v| Value::Int(i64::from(v))),
        "unsigned short" | "UInt16" => reader.u16().map(|v| Value::UInt(u64::from(v))),
        "int" | "SInt32" => reader.i32().map(|v| Value::Int(i64::from(v))),
        "unsigned int" | "UInt32" | "Type*" => reader.u32().map(|v| Value::UInt(u64::from(v))),
        "long long" | "SInt64" => reader.i64().map(Value::Int),
        "unsigned long long" | "UInt64" | "FileSize" => reader.u64().map(Value::UInt),
        "float" => reader.f32().map(|v| Value::Float(f64::from(v))),
        "double" => reader.f64().map(Value::Float),
        "string" => read_string(reader).map(Value::Str),
        "TypelessData" => reader.count().and_then(|count| reader.bytes(count)).map(|bytes| Value::Bytes(bytes.to_vec())),
        _ => return None,
    };
    Some(value)
}

fn read_string(reader: &mut Reader) -> Result<String> {
    let length = reader.count()?;
    let text = String::from_utf8_lossy(reader.bytes(length)?).into_owned();
    reader.align(4);
    Ok(text)
}

/// Read node `index`. `align_self` is false for the elements of an array whose element type is
/// aligned: such an array aligns once, after its last element (as Unity writes it).
fn read_value(nodes: &[Node], index: usize, reader: &mut Reader, align_self: bool) -> Result<Value> {
    let node = &nodes[index];
    let mut align = node.aligned();
    let value = if let Some(value) = read_primitive(&node.type_name, reader) {
        value?
    } else if node.type_name == "pair" && node.children.len() == 2 {
        let first = read_value(nodes, node.children[0], reader, true)?;
        let second = read_value(nodes, node.children[1], reader, true)?;
        Value::Pair(Box::new(first), Box::new(second))
    } else if let Some(&array) = node.children.first().filter(|child| nodes[**child].type_name == "Array") {
        // A vector: its Array child holds the size, then the element type.
        let array_node = &nodes[array];
        align |= array_node.aligned();
        let element = *array_node.children.get(1).ok_or_else(|| super::FormatError(format!("array {} has no element type", node.name)))?;
        let count = reader.count()?;
        let element_node = &nodes[element];
        if matches!(element_node.type_name.as_str(), "UInt8" | "SInt8" | "char") {
            let bytes = reader.bytes(count)?.to_vec();
            if element_node.aligned() {
                reader.align(4);
            }
            Value::Bytes(bytes)
        } else {
            let per_element = !element_node.aligned();
            let items = (0..count).map(|_| read_value(nodes, element, reader, per_element)).collect::<Result<Vec<_>>>()?;
            if element_node.aligned() {
                reader.align(4);
            }
            Value::List(items)
        }
    } else {
        Value::Object(node.children.iter().map(|&child| Ok((nodes[child].name.clone(), read_value(nodes, child, reader, true)?))).collect::<Result<Vec<_>>>()?)
    };
    if align && align_self {
        reader.align(4);
    }
    Ok(value)
}

#[cfg(test)]
#[path = "serialized_tests.rs"]
mod tests;
