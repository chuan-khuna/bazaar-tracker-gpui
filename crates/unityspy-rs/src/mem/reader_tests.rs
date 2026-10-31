use super::*;

#[test]
fn typed_reads_use_the_right_widths_and_signs() {
    let memory = BufferMemory::new();
    memory.write(0x1000, &[0xFF, 0xFF, 0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
    assert_eq!(memory.read_u8(0x1000).unwrap(), 255);
    assert_eq!(memory.read_i8(0x1000).unwrap(), -1);
    assert_eq!(memory.read_u16(0x1000).unwrap(), 0xFFFF);
    assert_eq!(memory.read_i16(0x1002).unwrap(), -2);
    assert_eq!(memory.read_i32(0x1004).unwrap(), -1);
    assert_eq!(memory.read_u32(0x1004).unwrap(), u32::MAX);
    assert_eq!(memory.read_u64(0x1000).unwrap(), 0xFFFF_FFFF_FFFE_FFFF);
    assert_eq!(memory.read_i64(0x1000).unwrap(), -65537);
}

#[test]
fn floats_read_all_their_bytes() {
    let memory = BufferMemory::new();
    memory.write_f32(0x2000, 1.5);
    memory.write_f64(0x2010, -2.25);
    assert_eq!(memory.read_f32(0x2000).unwrap(), 1.5);
    assert_eq!(memory.read_f64(0x2010).unwrap(), -2.25);
}

#[test]
fn a_read_outside_mapped_memory_is_an_error_naming_the_address() {
    let memory = BufferMemory::new();
    memory.write_u32(0x1000, 7);
    let error = memory.read_u32(0x5000).unwrap_err();
    assert!(matches!(error, Error::Read { address: 0x5000, len: 4, .. }), "{error}");
}

#[test]
fn a_partial_read_across_the_end_of_mapped_memory_is_an_error() {
    let memory = BufferMemory::new();
    memory.map(0x1000, 4096);
    assert!(memory.read_u64(0x1000 + 4092).is_err());
}

#[test]
fn a_c_string_at_the_end_of_a_mapped_page_is_read_without_overrunning() {
    let memory = BufferMemory::new();
    memory.map(0x1000, 4096);
    // "Data" and its NUL fill the last five bytes of the only mapped page.
    memory.write_c_string(0x1000 + 4096 - 5, "Data");
    assert_eq!(memory.read_c_string(0x1000 + 4096 - 5).unwrap(), "Data");
}

#[test]
fn a_c_string_longer_than_a_chunk_is_read_whole() {
    let memory = BufferMemory::new();
    let long = "x".repeat(700);
    memory.write_c_string(0x1000, &long);
    assert_eq!(memory.read_c_string(0x1000).unwrap(), long);
}

#[test]
fn a_c_string_without_an_end_is_rejected() {
    let memory = BufferMemory::new();
    memory.write(0x1000, &vec![b'a'; 3000]);
    assert!(matches!(memory.read_c_string(0x1000), Err(Error::Implausible { .. })));
}
