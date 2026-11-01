use super::*;

/// A type tree node record: (level, type, name, meta flag), with names in a local string buffer.
fn tree(nodes: &[(u8, &str, &str, u32)]) -> Vec<Node> {
    let mut strings = Vec::new();
    let mut offset = |text: &str| {
        let at = strings.len() as u32;
        strings.extend_from_slice(text.as_bytes());
        strings.push(0);
        at
    };
    let raw: Vec<(u8, u32, u32, u32)> = nodes.iter().map(|(level, ty, name, flag)| (*level, offset(ty), offset(name), *flag)).collect();
    let mut blob = Vec::new();
    blob.extend_from_slice(&(raw.len() as i32).to_le_bytes());
    blob.extend_from_slice(&(strings.len() as i32).to_le_bytes());
    for (level, ty, name, flag) in &raw {
        blob.extend_from_slice(&1u16.to_le_bytes());
        blob.push(*level);
        blob.push(0);
        blob.extend_from_slice(&ty.to_le_bytes());
        blob.extend_from_slice(&name.to_le_bytes());
        blob.extend_from_slice(&0i32.to_le_bytes());
        blob.extend_from_slice(&0i32.to_le_bytes());
        blob.extend_from_slice(&flag.to_le_bytes());
        blob.extend_from_slice(&0u64.to_le_bytes());
    }
    blob.extend_from_slice(&strings);
    read_type_tree(&mut Reader::new(&blob, false), 22).unwrap()
}

#[test]
fn a_class_with_a_string_an_aligned_bool_and_a_vector_of_pairs() {
    let nodes = tree(&[
        (0, "AssetBundle", "Base", 0),
        (1, "string", "m_Name", ALIGN),
        (2, "Array", "Array", ALIGN),
        (3, "int", "size", 0),
        (3, "char", "data", 0),
        (1, "bool", "m_Flag", ALIGN),
        (1, "vector", "m_Container", 0),
        (2, "Array", "Array", 0),
        (3, "int", "size", 0),
        (3, "pair", "data", 0),
        (4, "string", "first", ALIGN),
        (5, "Array", "Array", ALIGN),
        (6, "int", "size", 0),
        (6, "char", "data", 0),
        (4, "PPtr<Object>", "second", 0),
        (5, "int", "m_FileID", 0),
        (5, "SInt64", "m_PathID", 0),
    ]);
    let mut data = Vec::new();
    data.extend_from_slice(&5i32.to_le_bytes());
    data.extend_from_slice(b"cards\0\0\0"); // aligned to 4
    data.extend_from_slice(&[1, 0, 0, 0]); // the bool, then 3 bytes of alignment
    data.extend_from_slice(&1i32.to_le_bytes());
    data.extend_from_slice(&2i32.to_le_bytes());
    data.extend_from_slice(b"ab\0\0");
    data.extend_from_slice(&0i32.to_le_bytes());
    data.extend_from_slice(&42i64.to_le_bytes());

    let value = read_value(&nodes, 0, &mut Reader::new(&data, false), true).unwrap();
    assert_eq!(value.get("m_Name").and_then(Value::as_str), Some("cards"));
    assert_eq!(value.get("m_Flag"), Some(&Value::Bool(true)));
    let container = value.get("m_Container").and_then(Value::as_list).unwrap();
    let Value::Pair(path, target) = &container[0] else { panic!("a pair") };
    assert_eq!(path.as_str(), Some("ab"));
    assert_eq!(target.as_pptr(), Some((0, 42)));
}

#[test]
fn byte_vectors_come_back_as_bytes() {
    let nodes = tree(&[(0, "Texture2D", "Base", 0), (1, "vector", "image data", ALIGN), (2, "Array", "Array", 0), (3, "int", "size", 0), (3, "UInt8", "data", 0), (1, "int", "m_After", 0)]);
    let mut data = Vec::new();
    data.extend_from_slice(&3i32.to_le_bytes());
    data.extend_from_slice(&[7, 8, 9, 0]);
    data.extend_from_slice(&5i32.to_le_bytes());
    let value = read_value(&nodes, 0, &mut Reader::new(&data, false), true).unwrap();
    assert_eq!(value.get("image data").and_then(Value::as_bytes), Some(&[7u8, 8, 9][..]));
    assert_eq!(value.get("m_After").and_then(Value::as_i64), Some(5));
}

#[test]
fn common_strings_are_found_by_offset() {
    assert_eq!(tree_string(b"", 0x8000_0000), "AABB");
    assert_eq!(tree_string(b"mine\0", 0), "mine");
}
