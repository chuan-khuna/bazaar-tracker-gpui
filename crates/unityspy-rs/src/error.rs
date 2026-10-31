//! The one error type of the crate. Every missing process, module, export, assembly, class or
//! field is its own variant, and names what was missing.

use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    /// No running process has this name.
    ProcessNotFound(String),
    /// The process exists but could not be opened for reading.
    OpenProcess { pid: u32, code: u32 },
    /// The process has exited.
    ProcessGone,
    /// The process has no loaded module with this name (compared ignoring case).
    ModuleNotFound(String),
    /// The module does not export this function.
    ExportNotFound(String),
    /// The module's headers are not a PE file we can read.
    BadPe(&'static str),
    /// `mono_get_root_domain` does not start with the instruction bytes we expect.
    UnexpectedRootDomainCode { address: u64, bytes: [u8; 8] },
    /// The Mono runtime has not created its root domain yet.
    RootDomainNull,
    /// The domain has no assembly with this name.
    AssemblyNotFound(String),
    /// The game's executable has a Unity version we have no offset table for.
    UnsupportedUnityVersion(String),
    /// The image has no class with this name.
    ClassNotFound(String),
    /// The class (or its parents) has no field with this name.
    FieldNotFound { class: String, field: String },
    /// A static was read from a class the runtime has not initialised yet.
    NotInitialised(String),
    /// A static was asked for on a field that has no storage (an instance field or a `const`).
    NotAStaticField { class: String, field: String },
    /// An instance field was asked for on a static field.
    NotAnInstanceField { class: String, field: String },
    /// A value of this kind cannot be read.
    UnsupportedType(u8),
    /// A generic parameter (`T`) was met where the concrete type is needed.
    UnresolvedGeneric { class: String, field: String },
    /// A string or array length that cannot be real, so the pointer is probably stale.
    Implausible { what: &'static str, value: i64 },
    /// Following a list of pointers did not end.
    Loop(&'static str),
    /// A chain of reads met something that is not what it should be.
    Unexpected(String),
    /// Reading `len` bytes at `address` failed.
    Read { address: u64, len: usize, source: std::io::Error },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProcessNotFound(name) => write!(f, "process '{name}' is not running"),
            Self::OpenProcess { pid, code } => write!(f, "could not open process {pid} for reading (Windows error {code})"),
            Self::ProcessGone => write!(f, "the process has exited"),
            Self::ModuleNotFound(name) => write!(f, "module '{name}' is not loaded in the process"),
            Self::ExportNotFound(name) => write!(f, "the module does not export '{name}'"),
            Self::BadPe(why) => write!(f, "unreadable PE headers: {why}"),
            Self::UnexpectedRootDomainCode { address, bytes } => {
                write!(f, "mono_get_root_domain at {address:#x} starts with {bytes:02x?}, not the expected `mov rax,[rip+rel32]; ret`")
            }
            Self::RootDomainNull => write!(f, "the Mono root domain is not created yet"),
            Self::AssemblyNotFound(name) => write!(f, "assembly '{name}' is not loaded in the root domain"),
            Self::UnsupportedUnityVersion(version) => write!(f, "no Mono offsets for Unity version {version}"),
            Self::ClassNotFound(name) => write!(f, "class '{name}' not found in the image"),
            Self::FieldNotFound { class, field } => write!(f, "field '{field}' not found in class '{class}' or its parents"),
            Self::NotInitialised(class) => write!(f, "class '{class}' is not initialised yet, so it has no statics"),
            Self::NotAStaticField { class, field } => write!(f, "field '{field}' of '{class}' is not a static field with storage"),
            Self::NotAnInstanceField { class, field } => write!(f, "field '{field}' of '{class}' is static, so it has no value on an object"),
            Self::UnsupportedType(code) => write!(f, "cannot read values of type code {code:#04x}"),
            Self::UnresolvedGeneric { class, field } => write!(f, "field '{field}' of '{class}' has a generic parameter type, not a concrete one"),
            Self::Implausible { what, value } => write!(f, "implausible {what} {value}: corrupt or dangling pointer"),
            Self::Loop(what) => write!(f, "the {what} does not end: corrupt or changing memory"),
            Self::Unexpected(what) => write!(f, "unexpected memory contents: {what}"),
            Self::Read { address, len, source } => write!(f, "reading {len} bytes at {address:#x} failed: {source}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            _ => None,
        }
    }
}
