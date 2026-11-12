//! Product pictures of parts: the picture of an EDZ file is kept as a small JPEG
//! ([`openecraft_parts::Part::picture`]), so that parts databases and projects stay small.

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::RgbImage;

/// Largest width and height of a picture, px (a few KB each).
pub const SIZE: u32 = 192;

/// The picture (PNG, JPEG) as a JPEG of at most [`SIZE`] px on white, in base64;
/// `None` if it cannot be read.
pub fn thumbnail(data: &[u8]) -> Option<String> {
    let img = image::load_from_memory(data).ok()?;
    let img = if img.width() > SIZE || img.height() > SIZE { img.resize(SIZE, SIZE, FilterType::Triangle) } else { img };
    let rgba = img.to_rgba8();
    let white = |c: u8, a: u8| ((c as u16 * a as u16 + 255 * (255 - a as u16)) / 255) as u8;
    let rgb = RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [r, g, b, a] = rgba.get_pixel(x, y).0;
        image::Rgb([white(r, a), white(g, a), white(b, a)])
    });
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 80).encode_image(&rgb).ok()?;
    Some(openecraft_parts::base64::encode(&out))
}

/// Picture files by their extension.
pub fn is_picture(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [".png", ".jpg", ".jpeg"].iter().any(|e| name.ends_with(e))
}

#[cfg(test)]
mod tests;
