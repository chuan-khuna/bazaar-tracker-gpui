use super::*;
use crate::mem::reader::BufferMemory;

/// Where the image is loaded, and where it was linked (`__TEXT` at 0): the slide is `BASE`.
const BASE: u64 = 0x1_2000_0000;
const LINKEDIT_VMADDR: u64 = 0x30_0000;
const LINKEDIT_FILEOFF: u64 = 0x2F_0000;
const SYMOFF: u32 = 0x2F_0100;
const STROFF: u32 = 0x2F_1000;
const FUNCTION_VALUE: u64 = 0x11_C368;

fn segment(name: &str, vmaddr: u64, fileoff: u64) -> Vec<u8> {
    let mut command = Vec::new();
    command.extend_from_slice(&LC_SEGMENT_64.to_le_bytes());
    command.extend_from_slice(&72u32.to_le_bytes());
    let mut segment_name = [0u8; 16];
    segment_name[..name.len()].copy_from_slice(name.as_bytes());
    command.extend_from_slice(&segment_name);
    command.extend_from_slice(&vmaddr.to_le_bytes());
    command.extend_from_slice(&0x1000u64.to_le_bytes());
    command.extend_from_slice(&fileoff.to_le_bytes());
    command.extend_from_slice(&0x1000u64.to_le_bytes());
    command.resize(72, 0);
    command
}

fn nlist(string: u32, kind: u8, value: u64) -> Vec<u8> {
    let mut entry = Vec::new();
    entry.extend_from_slice(&string.to_le_bytes());
    entry.push(kind);
    entry.push(1);
    entry.extend_from_slice(&0u16.to_le_bytes());
    entry.extend_from_slice(&value.to_le_bytes());
    entry
}

/// A dylib loaded at `BASE` that exports `mono_get_root_domain` (and has a stab and an undefined
/// symbol of the same name in front of it, which must be skipped).
fn image(memory: &BufferMemory) {
    let mut commands = segment("__TEXT", 0, 0);
    commands.extend(segment("__LINKEDIT", LINKEDIT_VMADDR, LINKEDIT_FILEOFF));
    let mut symtab = Vec::new();
    for value in [LC_SYMTAB, 24, SYMOFF, 4, STROFF, 64] {
        symtab.extend_from_slice(&value.to_le_bytes());
    }
    commands.extend(symtab);
    let mut header = Vec::new();
    for value in [MH_MAGIC_64, 0x0100_000C, 0, 6, 3, commands.len() as u32, 0, 0] {
        header.extend_from_slice(&value.to_le_bytes());
    }
    header.extend(commands);
    memory.write(BASE, &header);

    let strings = b"\0_other\0_mono_get_root_domain\0";
    let mut symbols = nlist(1, 0x0F, 0x10);
    symbols.extend(nlist(8, 0x24, 0xDEAD)); // a stab
    symbols.extend(nlist(8, 0x01, 0xBEEF)); // undefined
    symbols.extend(nlist(8, 0x0F, FUNCTION_VALUE));
    let linkedit = BASE + LINKEDIT_VMADDR - LINKEDIT_FILEOFF;
    memory.write(linkedit + u64::from(SYMOFF), &symbols);
    let mut table = strings.to_vec();
    table.resize(64, 0);
    memory.write(linkedit + u64::from(STROFF), &table);
}

#[test]
fn an_export_is_found_and_moved_by_the_slide() {
    let memory = BufferMemory::new();
    image(&memory);
    assert_eq!(find_export(&memory, BASE, "mono_get_root_domain").unwrap(), BASE + FUNCTION_VALUE);
    assert_eq!(find_export(&memory, BASE, "other").unwrap(), BASE + 0x10);
}

#[test]
fn a_missing_export_is_named() {
    let memory = BufferMemory::new();
    image(&memory);
    let error = find_export(&memory, BASE, "mono_domain_get").unwrap_err();
    assert!(matches!(&error, Error::ExportNotFound(name) if name == "mono_domain_get"));
}

#[test]
fn a_prefix_of_a_name_is_not_a_match() {
    let memory = BufferMemory::new();
    image(&memory);
    assert!(find_export(&memory, BASE, "mono_get_root").is_err());
}

#[test]
fn something_that_is_not_mach_o_is_refused() {
    let memory = BufferMemory::new();
    memory.write(BASE, &[0x4D, 0x5A, 0, 0, 0, 0, 0, 0]);
    assert!(matches!(find_export(&memory, BASE, "mono_get_root_domain"), Err(Error::BadMachO(_))));
}
