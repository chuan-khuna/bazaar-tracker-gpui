use super::*;
use crate::mem::reader::BufferMemory;

const BASE: u64 = 0x1_0000_0000;

/// A PE32+ image with three named exports whose ordinals are `[2, 0, 1]`, so a name's index is
/// never its function index.
fn image() -> BufferMemory {
    let memory = BufferMemory::new();
    memory.write_u16(BASE, 0x5A4D);
    memory.write_u32(BASE + 0x3C, 0x80);
    memory.write_u32(BASE + 0x80, 0x0000_4550);
    memory.write_u16(BASE + 0x80 + 0x18, 0x20B);
    memory.write_u32(BASE + 0x80 + 0x88, 0x1000);
    memory.write_u32(BASE + 0x80 + 0x8C, 0x200);

    let directory = BASE + 0x1000;
    memory.write_u32(directory + 0x14, 3);
    memory.write_u32(directory + 0x18, 3);
    memory.write_u32(directory + 0x1C, 0x1100);
    memory.write_u32(directory + 0x20, 0x1200);
    memory.write_u32(directory + 0x24, 0x1300);

    for (i, rva) in [0x5000u32, 0x6000, 0x7000].into_iter().enumerate() {
        memory.write_u32(BASE + 0x1100 + 4 * i as u64, rva);
    }
    for (i, (name, ordinal)) in [("alpha", 2u16), ("mono_get_root_domain", 0), ("zeta", 1)].into_iter().enumerate() {
        let name_rva = 0x1400 + 0x40 * i as u64;
        memory.write_u32(BASE + 0x1200 + 4 * i as u64, name_rva as u32);
        memory.write_c_string(BASE + name_rva, name);
        memory.write_u16(BASE + 0x1300 + 2 * i as u64, ordinal);
    }
    memory
}

#[test]
fn an_export_is_resolved_through_its_ordinal_not_its_name_index() {
    let memory = image();
    // The name is at index 1, but its ordinal is 0, so the function is the first one.
    assert_eq!(find_export(&memory, BASE, "mono_get_root_domain").unwrap(), BASE + 0x5000);
    assert_eq!(find_export(&memory, BASE, "alpha").unwrap(), BASE + 0x7000);
    assert_eq!(find_export(&memory, BASE, "zeta").unwrap(), BASE + 0x6000);
}

#[test]
fn a_missing_export_is_named_in_the_error() {
    let memory = image();
    let error = find_export(&memory, BASE, "mono_get_root").unwrap_err();
    assert!(matches!(&error, Error::ExportNotFound(name) if name == "mono_get_root"));
    assert!(matches!(find_export(&memory, BASE, "mono_get_root_domain_x"), Err(Error::ExportNotFound(_))));
}

#[test]
fn a_module_that_is_not_a_pe_file_is_rejected() {
    let memory = BufferMemory::new();
    memory.write_u16(BASE, 0x1234);
    assert!(matches!(find_export(&memory, BASE, "x"), Err(Error::BadPe(_))));
}

#[test]
fn a_module_with_no_export_directory_has_no_exports() {
    let memory = image();
    memory.write_u32(BASE + 0x80 + 0x88, 0);
    assert!(matches!(find_export(&memory, BASE, "alpha"), Err(Error::ExportNotFound(_))));
}

#[test]
fn only_the_headers_and_export_tables_are_read() {
    // Everything outside those pages is unmapped, so reading the whole image would fail.
    let memory = image();
    assert!(find_export(&memory, BASE, "zeta").is_ok());
    assert!(memory.read_u8(BASE + 0x5000).is_err());
}
