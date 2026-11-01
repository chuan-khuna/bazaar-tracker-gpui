//! Just enough of Unity's file formats to read card art out of The Bazaar's asset bundles: the
//! UnityFS bundle ([`bundle`]), the serialized file inside it with its type trees ([`serialized`]),
//! and Texture2D pixels ([`texture`]). Layouts follow UnityPy 1.25, which reads this game's
//! bundles; docs/rea/the-bazaar/card-art.md has what the game stores where.

pub mod bundle;
mod common_strings;
pub mod serialized;
pub mod texture;

use std::fmt;

/// What went wrong reading a Unity file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatError(pub String);

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FormatError {}

pub type Result<T> = std::result::Result<T, FormatError>;

pub(crate) fn error<T>(message: impl Into<String>) -> Result<T> {
    Err(FormatError(message.into()))
}

/// A cursor over bytes, in either byte order.
pub(crate) struct Reader<'a> {
    data: &'a [u8],
    pub pos: usize,
    pub big_endian: bool,
}

macro_rules! read_number {
    ($name:ident, $ty:ty) => {
        pub fn $name(&mut self) -> Result<$ty> {
            let bytes: [u8; size_of::<$ty>()] = self.bytes(size_of::<$ty>())?.try_into().expect("the length was asked for");
            Ok(if self.big_endian { <$ty>::from_be_bytes(bytes) } else { <$ty>::from_le_bytes(bytes) })
        }
    };
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8], big_endian: bool) -> Self {
        Self { data, pos: 0, big_endian }
    }

    pub fn bytes(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(count).filter(|end| *end <= self.data.len());
        match end {
            Some(end) => {
                let slice = &self.data[self.pos..end];
                self.pos = end;
                Ok(slice)
            }
            None => error(format!("read past the end ({count} bytes at {} of {})", self.pos, self.data.len())),
        }
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?[0])
    }

    read_number!(i16, i16);
    read_number!(u16, u16);
    read_number!(i32, i32);
    read_number!(u32, u32);
    read_number!(i64, i64);
    read_number!(u64, u64);
    read_number!(f32, f32);
    read_number!(f64, f64);

    /// A count that must not be negative or larger than what is left.
    pub fn count(&mut self) -> Result<usize> {
        let count = self.i32()?;
        if count < 0 || count as usize > self.data.len().saturating_sub(self.pos) {
            return error(format!("bad count {count} at {}", self.pos - 4));
        }
        Ok(count as usize)
    }

    pub fn cstring(&mut self) -> Result<String> {
        let rest = &self.data[self.pos.min(self.data.len())..];
        let Some(end) = rest.iter().position(|byte| *byte == 0) else {
            return error("unterminated string");
        };
        let text = String::from_utf8_lossy(&rest[..end]).into_owned();
        self.pos += end + 1;
        Ok(text)
    }

    pub fn align(&mut self, to: usize) {
        self.pos = self.pos.div_ceil(to) * to;
    }
}
