//! UnityFS asset bundles: a header, a block and directory table, then the files packed in it as
//! a run of (usually LZ4) compressed blocks. The game's bundles are format 8, LZ4HC, with flags
//! 0x243 and the engine version stripped to "0.0.0" (card-art.md §1.3).

use super::{Reader, Result, error};

/// Bits of the archive flags.
const COMPRESSION_MASK: u32 = 0x3f;
const BLOCKS_INFO_AT_END: u32 = 0x80;
const NEEDS_PADDING_AT_START: u32 = 0x200;
const ENCRYPTED: u32 = 0x400;

/// A file packed in a bundle: a serialized file (`CAB-<hash>`) or its texture data (`.resS`).
pub struct Entry {
    pub path: String,
    pub data: Vec<u8>,
}

/// Unpack every file in a UnityFS bundle.
pub fn read(bytes: &[u8]) -> Result<Vec<Entry>> {
    let mut reader = Reader::new(bytes, true);
    if reader.cstring()? != "UnityFS" {
        return error("not a UnityFS bundle");
    }
    let format = reader.u32()?;
    let _player_version = reader.cstring()?;
    let _engine_version = reader.cstring()?;
    let _size = reader.i64()?;
    let info_compressed = reader.u32()? as usize;
    let info_size = reader.u32()? as usize;
    let flags = reader.u32()?;
    if flags & ENCRYPTED != 0 {
        return error("the bundle is encrypted");
    }
    if format >= 7 {
        reader.align(16);
    }

    let info_bytes = if flags & BLOCKS_INFO_AT_END != 0 {
        let start = bytes.len().checked_sub(info_compressed).ok_or_else(|| super::FormatError("bad block info size".into()))?;
        &bytes[start..]
    } else {
        reader.bytes(info_compressed)?
    };
    let info = decompress(info_bytes, info_size, flags & COMPRESSION_MASK)?;
    let mut info = Reader::new(&info, true);
    info.bytes(16)?; // hash of the uncompressed data
    let blocks = (0..info.count()?).map(|_| Ok((info.u32()? as usize, info.u32()? as usize, info.u16()?))).collect::<Result<Vec<_>>>()?;
    let nodes = (0..info.count()?).map(|_| Ok((info.i64()? as usize, info.i64()? as usize, info.u32()?, info.cstring()?))).collect::<Result<Vec<_>>>()?;

    if flags & NEEDS_PADDING_AT_START != 0 {
        reader.align(16);
    }
    let total: usize = blocks.iter().map(|(size, _, _)| size).sum();
    let mut data = Vec::with_capacity(total);
    for (size, compressed, block_flags) in blocks {
        let block = reader.bytes(compressed)?;
        data.extend_from_slice(&decompress(block, size, u32::from(block_flags) & COMPRESSION_MASK)?);
    }

    nodes
        .into_iter()
        .map(|(offset, size, _flags, path)| match data.get(offset..offset + size) {
            Some(slice) => Ok(Entry { path, data: slice.to_vec() }),
            None => error(format!("{path} lies outside the bundle's data")),
        })
        .collect()
}

fn decompress(data: &[u8], size: usize, compression: u32) -> Result<Vec<u8>> {
    match compression {
        0 => Ok(data.to_vec()),
        2 | 3 => lz4_flex::block::decompress(data, size).map_err(|why| super::FormatError(format!("LZ4: {why}"))),
        1 => error("LZMA bundles are not supported"),
        other => error(format!("unknown compression {other}")),
    }
}
