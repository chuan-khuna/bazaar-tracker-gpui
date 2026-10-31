//! Texture2D pixels: found in the object or in the bundle's `.resS` stream, decoded from the
//! formats the game uses (card-art.md §1.4) into RGBA, top row first.

use image::RgbaImage;

use super::serialized::Value;
use super::{Result, error};

/// `m_TextureFormat` values.
mod format {
    pub const ALPHA8: i64 = 1;
    pub const RGB24: i64 = 3;
    pub const RGBA32: i64 = 4;
    pub const ARGB32: i64 = 5;
    pub const DXT1: i64 = 10;
    pub const DXT5: i64 = 12;
    pub const BGRA32: i64 = 14;
    pub const BC7: i64 = 25;
    pub const DXT1_CRUNCHED: i64 = 28;
    pub const DXT5_CRUNCHED: i64 = 29;
}

/// Decode a Texture2D. `stream` finds a `.resS` file of the bundle by the name in `m_StreamData.path`.
pub fn decode<'a>(texture: &Value, stream: impl Fn(&str) -> Option<&'a [u8]>) -> Result<RgbaImage> {
    let number = |name: &str| texture.get(name).and_then(Value::as_i64);
    let (Some(width), Some(height), Some(format)) = (number("m_Width"), number("m_Height"), number("m_TextureFormat")) else {
        return error("not a Texture2D");
    };
    let (width, height) = (width as usize, height as usize);
    if width == 0 || height == 0 || width > 16384 || height > 16384 {
        return error(format!("bad texture size {width}×{height}"));
    }

    let inline = texture.get("image data").and_then(Value::as_bytes).unwrap_or_default();
    let data: &[u8] = if inline.is_empty() {
        let streamed = texture.get("m_StreamData");
        let path = streamed.and_then(|s| s.get("path")).and_then(Value::as_str).unwrap_or_default();
        let offset = streamed.and_then(|s| s.get("offset")).and_then(Value::as_i64).unwrap_or(0) as usize;
        let size = streamed.and_then(|s| s.get("size")).and_then(Value::as_i64).unwrap_or(0) as usize;
        let name = path.rsplit('/').next().unwrap_or(path);
        let Some(file) = stream(name) else {
            return error(format!("texture data {path} is not in the bundle"));
        };
        match file.get(offset..offset + size) {
            Some(slice) => slice,
            None => return error(format!("texture data runs past the end of {name}")),
        }
    } else {
        inline
    };

    let pixels = width * height;
    let mut image = vec![0u32; pixels];
    let decoded = match format {
        format::BC7 => texture2ddecoder::decode_bc7(data, width, height, &mut image),
        format::DXT1 => texture2ddecoder::decode_bc1(data, width, height, &mut image),
        format::DXT5 => texture2ddecoder::decode_bc3(data, width, height, &mut image),
        format::DXT1_CRUNCHED | format::DXT5_CRUNCHED => texture2ddecoder::decode_unity_crunch(data, width, height, &mut image),
        format::RGBA32 | format::ARGB32 | format::BGRA32 | format::RGB24 | format::ALPHA8 => return plain(data, width, height, format),
        other => return error(format!("texture format {other} is not supported")),
    };
    decoded.map_err(|why| super::FormatError(format!("texture format {format}: {why}")))?;
    // The decoder writes each pixel as the little-endian bytes B, G, R, A.
    let mut rgba = Vec::with_capacity(pixels * 4);
    for pixel in image {
        let [b, g, r, a] = pixel.to_le_bytes();
        rgba.extend_from_slice(&[r, g, b, a]);
    }
    Ok(flip(RgbaImage::from_raw(width as u32, height as u32, rgba).expect("the buffer has width × height pixels")))
}

/// Uncompressed formats.
fn plain(data: &[u8], width: usize, height: usize, format: i64) -> Result<RgbaImage> {
    let size = match format {
        format::RGB24 => 3,
        format::ALPHA8 => 1,
        _ => 4,
    };
    let Some(data) = data.get(..width * height * size) else {
        return error("texture data is too short");
    };
    let rgba: Vec<u8> = data
        .chunks_exact(size)
        .flat_map(|p| match format {
            format::RGB24 => [p[0], p[1], p[2], 255],
            format::ALPHA8 => [255, 255, 255, p[0]],
            format::ARGB32 => [p[1], p[2], p[3], p[0]],
            format::BGRA32 => [p[2], p[1], p[0], p[3]],
            _ => [p[0], p[1], p[2], p[3]],
        })
        .collect();
    Ok(flip(RgbaImage::from_raw(width as u32, height as u32, rgba).expect("the buffer has width × height pixels")))
}

/// Unity stores the bottom row first.
fn flip(image: RgbaImage) -> RgbaImage {
    image::imageops::flip_vertical(&image)
}
