//! The Addressables binary content catalog (`StreamingAssets/aa/catalog.bin`), which maps a card's
//! `ArtKey` to the asset path and the bundle that holds it. The format is in card-art.md §2.2:
//! every offset points at data whose byte length is the u32 just before it.

use std::collections::HashMap;

/// Where an addressable key's asset is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    /// The asset's path in its bundle, as the bundle's container lists it.
    pub asset: String,
    /// The bundle's file name, in `StandaloneWindows64`.
    pub bundle: String,
}

const MAGIC: u32 = 0x0DE3_8942;
const NONE: u32 = 0xFFFF_FFFF;

/// String keys to their locations.
#[derive(Debug, Default)]
pub struct Catalog {
    keys: HashMap<String, Location>,
}

impl Catalog {
    pub fn parse(buffer: &[u8]) -> Result<Self, String> {
        let file = File(buffer);
        if file.u32(0)? != MAGIC {
            return Err("not an Addressables binary catalog".into());
        }
        let keys_offset = file.u32(8)?;
        let mut keys = HashMap::new();
        for index in 0..file.size(keys_offset)? / 8 {
            let entry = keys_offset + index * 8;
            // A key that cannot be read is skipped: the catalog has other kinds of keys too.
            let Ok(Some(key)) = file.key(file.u32(entry)?) else { continue };
            let locations = file.u32(entry + 4)?;
            if file.size(locations)? < 4 {
                continue;
            }
            if let Ok(Some(location)) = file.location(file.u32(locations)?) {
                keys.entry(key.replace('/', "_")).or_insert_with(|| location.clone());
                keys.entry(key).or_insert(location);
            }
        }
        Ok(Self { keys })
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Where an `ArtKey` points: as written, without its extension, or with `.png` added.
    pub fn resolve(&self, art_key: &str) -> Option<&Location> {
        let stem = art_key.rsplit_once('.').map_or(art_key, |(stem, _)| stem);
        [art_key.to_owned(), stem.to_owned(), format!("{art_key}.png")].iter().find_map(|candidate| self.keys.get(candidate))
    }
}

struct File<'a>(&'a [u8]);

impl File<'_> {
    fn u32(&self, offset: u32) -> Result<u32, String> {
        let at = offset as usize;
        self.0.get(at..at + 4).map(|bytes| u32::from_le_bytes(bytes.try_into().expect("four bytes"))).ok_or_else(|| format!("catalog offset {offset} is past the end"))
    }

    /// The byte length of the data at `offset`.
    fn size(&self, offset: u32) -> Result<u32, String> {
        self.u32(offset.checked_sub(4).ok_or("catalog offset 0 has no size")?)
    }

    /// A string by its id: bit 31 marks UTF-16, bit 30 a "dynamic" string stored as a linked list
    /// of parts, last part first, to be joined with `separator`.
    fn string(&self, id: u32, separator: &str) -> Result<Option<String>, String> {
        if id == NONE {
            return Ok(None);
        }
        let offset = id & 0x3fff_ffff;
        if id & 0x4000_0000 != 0 {
            let mut parts = Vec::new();
            let mut node = offset;
            loop {
                parts.push(self.string(self.u32(node)?, "")?.unwrap_or_default());
                let next = self.u32(node + 4)?;
                if next == NONE || parts.len() > 256 {
                    break;
                }
                node = next;
            }
            parts.reverse();
            return Ok(Some(parts.join(separator)));
        }
        let start = offset as usize;
        let bytes = self.0.get(start..start + self.size(offset)? as usize).ok_or("catalog string is past the end")?;
        Ok(Some(if id & 0x8000_0000 != 0 {
            let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|pair| u16::from_le_bytes(*pair)).collect();
            String::from_utf16_lossy(&units)
        } else {
            String::from_utf8_lossy(bytes).into_owned()
        }))
    }

    /// A key's text, if it is a string key.
    fn key(&self, offset: u32) -> Result<Option<String>, String> {
        let (type_id, object) = (self.u32(offset)?, self.u32(offset + 4)?);
        let class = self.string(self.u32(type_id + 4)?, ".")?.unwrap_or_default();
        if !class.ends_with("String") {
            return Ok(None);
        }
        self.string(self.u32(object)?, "/")
    }

    /// A location: `{primaryKey, internalId, providerId, dependencies, dependencyHash, extraData,
    /// type}`. A bundled asset's first dependency is its own bundle.
    fn location(&self, offset: u32) -> Result<Option<Location>, String> {
        let internal = self.u32(offset + 4)?;
        let dependencies = self.u32(offset + 12)?;
        if dependencies == NONE || self.size(dependencies)? < 4 {
            return Ok(None);
        }
        let bundle_location = self.u32(dependencies)?;
        let bundle_path = self.string(self.u32(bundle_location + 4)?, "/")?.unwrap_or_default();
        let bundle = bundle_path.rsplit(['/', '\\']).next().unwrap_or_default().to_owned();
        if !bundle.ends_with(".bundle") {
            return Ok(None);
        }
        let Some(asset) = self.string(internal, "/")? else { return Ok(None) };
        Ok(Some(Location { asset, bundle }))
    }
}
