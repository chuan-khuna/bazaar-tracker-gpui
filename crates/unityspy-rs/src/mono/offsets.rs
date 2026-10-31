//! Where the members of Mono's structures sit, which differs between Unity versions. Only 64-bit
//! Windows (PE) tables exist. The table is chosen by the game executable's version and never
//! guessed: an unknown version is an error, not a silent fallback.
//!
//! Values are from `docs/rea/unityspy/offsets.md` §1 (the expressions, not UnitySpy's stale comments).

use crate::error::{Error, Result};

/// The offsets of one Unity line's Mono runtime. Member names are inferred from Unity's Mono headers.
#[derive(Debug, PartialEq, Eq)]
pub struct MonoOffsets {
    /// Which Unity versions this table is for, for messages.
    pub name: &'static str,
    /// Whether the numbers were checked against a real game (UnitySpy checked the 2021/2022 ones;
    /// the Unity 6 ones are carried over unverified).
    pub verified: bool,
    /// `MonoAssembly.image`.
    pub assembly_image: u64,
    /// `MonoDomain.domain_assemblies`.
    pub domain_assemblies: u64,
    /// `MonoImage.class_cache`.
    pub image_class_cache: u64,
    /// `MonoInternalHashTable.size`.
    pub hash_table_size: u64,
    /// `MonoInternalHashTable.table`.
    pub hash_table_table: u64,
    /// `sizeof(MonoClassField)`.
    pub class_field_size: u64,
    /// The `MonoClass` flag word (`valuetype` 0x4, `enumtype` 0x8).
    pub class_bit_fields: u64,
    /// `MonoClass.class_kind` (a byte, `& 7`).
    pub class_kind: u64,
    pub class_parent: u64,
    pub class_nested_in: u64,
    pub class_name: u64,
    pub class_namespace: u64,
    pub class_vtable_size: u64,
    /// `MonoClass.sizes` (`instance_size` / `element_size` / `class_size`).
    pub class_size: u64,
    pub class_fields: u64,
    /// `MonoClass._byval_arg`, an inline `MonoType` describing the class as a value.
    pub class_byval_arg: u64,
    /// `MonoClass.runtime_info`.
    pub class_runtime_info: u64,
    /// `MonoClassGenericInst.generic_class`.
    pub class_generic_class: u64,
    /// `MonoGenericClass.cached_class`: the instantiated class, once the runtime has made it.
    pub generic_class_cached_class: u64,
    /// `MonoClassDef.field_count`.
    pub class_field_count: u64,
    /// `MonoClassDef.next_class_cache`.
    pub class_next_class_cache: u64,
    /// `MonoClassRuntimeInfo.domain_vtables[0]`.
    pub runtime_info_domain_vtables: u64,
    /// Where the static-data slot starts relative to `MonoVTable.vtable[]`.
    pub vtable: u64,
}

/// Unity 2021.3 and 2022.3, x64, PE. Checked against the original tracker's use of it.
pub const UNITY_2021_2022_X64: MonoOffsets = MonoOffsets {
    name: "Unity 2021.3/2022.3 x64",
    verified: true,
    assembly_image: 0x60,
    domain_assemblies: 0xA0,
    image_class_cache: 0x4D0,
    hash_table_size: 0x18,
    hash_table_table: 0x20,
    class_field_size: 0x20,
    class_bit_fields: 0x20,
    class_kind: 0x1B,
    class_parent: 0x30,
    class_nested_in: 0x38,
    class_name: 0x48,
    class_namespace: 0x50,
    class_vtable_size: 0x5C,
    class_size: 0x90,
    class_fields: 0x98,
    class_byval_arg: 0xB8,
    class_runtime_info: 0xD0,
    class_generic_class: 0xF0,
    generic_class_cached_class: 0x20,
    class_field_count: 0x100,
    class_next_class_cache: 0x108,
    runtime_info_domain_vtables: 0x8,
    vtable: 0x48,
};

/// Unity 6 (6000.x), x64, PE. The BazaarDBTracker fork lists 6000.0.58 under the 2021/2022 numbers
/// without checking them, so this table has the same values and is marked unverified until
/// `just spike` has been run against a Unity 6 game.
pub const UNITY_6_X64: MonoOffsets = MonoOffsets { name: "Unity 6 x64 (unverified)", verified: false, ..UNITY_2021_2022_X64 };

/// A Unity version, such as 2022.3.42 (the `f1` suffix is dropped).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnityVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl UnityVersion {
    pub fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self { major, minor, patch }
    }

    /// Parse `2022.3.42f1`, `6000.0.58` and the like. Only the leading digits of each part count.
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.trim().split('.').map(|part| {
            let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
            digits.parse::<u16>().ok()
        });
        Some(Self::new(parts.next()??, parts.next()??, parts.next().flatten().unwrap_or(0)))
    }
}

impl std::fmt::Display for UnityVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// The offset table for a game built with this Unity version, or an error naming the version.
pub fn for_version(version: UnityVersion) -> Result<&'static MonoOffsets> {
    match version.major {
        2021 | 2022 => Ok(&UNITY_2021_2022_X64),
        6000.. => Ok(&UNITY_6_X64),
        _ => Err(Error::UnsupportedUnityVersion(version.to_string())),
    }
}

#[cfg(test)]
#[path = "offsets_tests.rs"]
mod tests;
