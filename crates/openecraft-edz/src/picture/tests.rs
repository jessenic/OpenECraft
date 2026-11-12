use super::*;
use image::{ImageFormat, Rgba, RgbaImage};
use std::io::Cursor;

fn png(w: u32, h: u32, pixel: [u8; 4]) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    RgbaImage::from_pixel(w, h, Rgba(pixel)).write_to(&mut out, ImageFormat::Png).unwrap();
    out.into_inner()
}

fn decode(b64: &str) -> image::DynamicImage {
    image::load_from_memory(&openecraft_parts::base64::decode(b64).unwrap()).unwrap()
}

#[test]
fn large_pictures_shrink_to_a_small_jpeg() {
    let big = png(1000, 500, [200, 0, 0, 255]);
    let small = thumbnail(&big).unwrap();
    let img = decode(&small);
    assert_eq!((img.width(), img.height()), (SIZE, SIZE / 2), "the aspect ratio is kept");
    assert!(small.len() < big.len());
    // Small pictures keep their size.
    assert_eq!(decode(&thumbnail(&png(40, 30, [0, 0, 0, 255])).unwrap()).width(), 40);
}

#[test]
fn transparency_becomes_white() {
    let [r, g, b] = decode(&thumbnail(&png(8, 8, [0, 0, 0, 0])).unwrap()).to_rgb8().get_pixel(4, 4).0;
    assert!(r > 245 && g > 245 && b > 245);
}

#[test]
fn other_data_is_no_picture() {
    assert!(thumbnail(b"PNG").is_none());
    assert!(is_picture("a/LOGO.PNG") && is_picture("x.jpeg") && !is_picture("macro.ema"));
}
