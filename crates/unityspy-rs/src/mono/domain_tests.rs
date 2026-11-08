use super::*;
use crate::mem::reader::BufferMemory;
use crate::mono::offsets::UNITY_2021_2022_X64 as OFFSETS;

const FUNCTION: u64 = 0x7FF0_0000_1000;
const GLOBAL: u64 = 0x7FF0_0009_0000;
const DOMAIN: u64 = 0x2000_0000;

/// `mov rax,[rip+rel32]; ret` whose global is at `GLOBAL`.
fn root_domain_function(memory: &BufferMemory) {
    let rel32 = (GLOBAL - (FUNCTION + 7)) as i32;
    let mut code = vec![0x48, 0x8B, 0x05];
    code.extend_from_slice(&rel32.to_le_bytes());
    code.push(0xC3);
    memory.write(FUNCTION, &code);
    memory.write_u64(GLOBAL, DOMAIN);
}

#[test]
fn the_root_domain_is_decoded_from_good_bytes() {
    let memory = BufferMemory::new();
    root_domain_function(&memory);
    assert_eq!(root_domain(&memory, FUNCTION).unwrap(), DOMAIN);
}

#[test]
fn a_negative_displacement_is_followed_backwards() {
    let memory = BufferMemory::new();
    let function = 0x7FF0_0010_0000u64;
    let global = function - 0x400;
    let rel32 = (global as i64 - (function as i64 + 7)) as i32;
    let mut code = vec![0x48, 0x8B, 0x05];
    code.extend_from_slice(&rel32.to_le_bytes());
    code.push(0xC3);
    memory.write(function, &code);
    memory.write_u64(global, DOMAIN);
    assert_eq!(root_domain(&memory, function).unwrap(), DOMAIN);
}

#[test]
fn bad_opcode_bytes_are_rejected_before_the_displacement_is_used() {
    for (index, byte) in [(0, 0x49), (1, 0x8D), (2, 0x04), (7, 0xCC)] {
        let memory = BufferMemory::new();
        root_domain_function(&memory);
        memory.write_u8(FUNCTION + index, byte);
        let error = root_domain(&memory, FUNCTION).unwrap_err();
        assert!(matches!(error, Error::UnexpectedRootDomainCode { address: FUNCTION, .. }), "byte {index}: {error}");
    }
}

#[test]
fn a_jump_thunk_is_not_mistaken_for_the_function() {
    let memory = BufferMemory::new();
    memory.write(FUNCTION, &[0xE9, 0x10, 0x00, 0x00, 0x00, 0xCC, 0xCC, 0xCC]);
    assert!(matches!(root_domain(&memory, FUNCTION), Err(Error::UnexpectedRootDomainCode { .. })));
}

/// `adrp x8, page; ldr x0, [x8, #0xe98]; ret`, as in the macOS arm64 `libmonobdwgc-2.0.dylib`.
fn arm64_function(memory: &BufferMemory, function: u64, global: u64) {
    let pages = ((global & !0xFFF) as i64 - (function & !0xFFF) as i64) >> 12;
    let immediate = (pages as u32) & 0x1F_FFFF;
    let adrp = 0x9000_0000 | ((immediate & 0x3) << 29) | ((immediate >> 2) << 5) | 8;
    let ldr = 0xF940_0000 | ((((global & 0xFFF) / 8) as u32) << 10) | (8 << 5);
    let mut code = Vec::new();
    for instruction in [adrp, ldr, 0xD65F_03C0] {
        code.extend_from_slice(&u32::to_le_bytes(instruction));
    }
    memory.write(function, &code);
    memory.write_u64(global, DOMAIN);
}

#[test]
fn the_arm64_form_is_decoded() {
    let memory = BufferMemory::new();
    // The real numbers: the function at 0x11c368 loads the global at 0x30ee98.
    let base = 0x1_0400_0000u64;
    arm64_function(&memory, base + 0x11_C368, base + 0x30_EE98);
    assert_eq!(root_domain(&memory, base + 0x11_C368).unwrap(), DOMAIN);
}

#[test]
fn an_arm64_global_before_the_function_is_found() {
    let memory = BufferMemory::new();
    let function = 0x1_0480_0010u64;
    arm64_function(&memory, function, 0x1_0400_2008);
    assert_eq!(root_domain(&memory, function).unwrap(), DOMAIN);
}

#[test]
fn an_arm64_load_into_another_register_is_refused() {
    let memory = BufferMemory::new();
    arm64_function(&memory, FUNCTION, GLOBAL);
    // ldr x1 instead of ldr x0.
    let ldr = u32::from_le_bytes([0, 0, 0, 0]) | 0xF940_0000 | (8 << 5) | 1;
    memory.write(FUNCTION + 4, &ldr.to_le_bytes());
    assert!(matches!(root_domain(&memory, FUNCTION), Err(Error::UnexpectedRootDomainCode { .. })));
}

#[test]
fn the_macos_x64_form_with_a_frame_is_decoded() {
    let memory = BufferMemory::new();
    let rel32 = (GLOBAL - (FUNCTION + 4 + 7)) as i32;
    let mut code = vec![0x55, 0x48, 0x89, 0xE5, 0x48, 0x8B, 0x05];
    code.extend_from_slice(&rel32.to_le_bytes());
    code.extend_from_slice(&[0x5D, 0xC3]);
    memory.write(FUNCTION, &code);
    memory.write_u64(GLOBAL, DOMAIN);
    assert_eq!(root_domain(&memory, FUNCTION).unwrap(), DOMAIN);
}

#[test]
fn a_domain_that_is_not_created_yet_is_its_own_error() {
    let memory = BufferMemory::new();
    root_domain_function(&memory);
    memory.write_u64(GLOBAL, 0);
    assert!(matches!(root_domain(&memory, FUNCTION), Err(Error::RootDomainNull)));
}

/// A domain whose assembly list holds the named assemblies.
fn domain_with(memory: &BufferMemory, names: &[&str]) {
    let list_head = 0x3000_0000u64;
    memory.write_u64(DOMAIN + OFFSETS.domain_assemblies, if names.is_empty() { 0 } else { list_head });
    for (i, name) in names.iter().enumerate() {
        let node = list_head + 0x100 * i as u64;
        let assembly = 0x4000_0000 + 0x1000 * i as u64;
        let name_at = 0x5000_0000 + 0x100 * i as u64;
        memory.write_u64(node, assembly);
        memory.write_u64(node + 8, if i + 1 < names.len() { node + 0x100 } else { 0 });
        memory.write_u64(assembly + 16, name_at);
        memory.write_c_string(name_at, name);
        memory.write_u64(assembly + OFFSETS.assembly_image, 0x6000_0000 + 0x10000 * i as u64);
    }
}

#[test]
fn the_assembly_list_is_walked_with_each_name_and_image() {
    let memory = BufferMemory::new();
    domain_with(&memory, &["mscorlib", "UnityEngine.CoreModule", "TheBazaarRuntime"]);
    let found = assemblies(&memory, &OFFSETS, DOMAIN).unwrap();
    let names: Vec<_> = found.iter().map(|assembly| assembly.name.as_str()).collect();
    assert_eq!(names, ["mscorlib", "UnityEngine.CoreModule", "TheBazaarRuntime"]);
    assert_eq!(found[2].image, 0x6002_0000);
}

#[test]
fn an_assembly_is_found_by_exact_name() {
    let memory = BufferMemory::new();
    domain_with(&memory, &["mscorlib", "TheBazaarRuntime"]);
    assert_eq!(find_assembly(&memory, &OFFSETS, DOMAIN, "TheBazaarRuntime").unwrap().image, 0x6001_0000);
    let error = find_assembly(&memory, &OFFSETS, DOMAIN, "thebazaarruntime").unwrap_err();
    assert!(matches!(&error, Error::AssemblyNotFound(name) if name == "thebazaarruntime"));
}

#[test]
fn an_empty_assembly_list_finds_nothing() {
    let memory = BufferMemory::new();
    domain_with(&memory, &[]);
    assert!(assemblies(&memory, &OFFSETS, DOMAIN).unwrap().is_empty());
}

#[test]
fn a_circular_assembly_list_is_an_error_not_an_endless_walk() {
    let memory = BufferMemory::new();
    domain_with(&memory, &["mscorlib"]);
    memory.write_u64(0x3000_0000 + 8, 0x3000_0000);
    assert!(matches!(assemblies(&memory, &OFFSETS, DOMAIN), Err(Error::Loop(_))));
}
