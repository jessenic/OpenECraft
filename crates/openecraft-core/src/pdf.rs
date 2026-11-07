//! PDF export of all pages (each in the paper format of its plot frame): vector
//! graphics, Helvetica text, clickable cross references and report rows, one bookmark
//! per page; the files of document pages are attached to their page. Texts with
//! characters beyond WinAnsi (Polish, Czech, Turkish, Greek, Cyrillic …) use a subset
//! of the bundled Ubuntu font, embedded with a ToUnicode map so they can be searched
//! and copied; characters it lacks come from a system fallback font or the bundled
//! Chinese, Japanese and Korean fonts. The file is plain ASCII (non-ASCII text is written as WinAnsi octal
//! escapes, attachments as hex, the font compressed and ASCII85 encoded).

use crate::geom::{p, P};
use crate::model::{PageKind, Project};
use crate::render::{draw_page, Draw, Ink, Opts, TextStyle, VAnchor};
use crate::analysis::Analysis;
use crate::symbols::Anchor;
use crate::ttf::Font;
use openecraft_fonts::Fallback;
use std::collections::BTreeMap;
use std::fmt::Write;
use std::sync::OnceLock;

/// The embedded font for texts beyond WinAnsi.
fn unicode_font() -> Option<&'static Font<'static>> {
    static FONT: OnceLock<Option<Font<'static>>> = OnceLock::new();
    FONT.get_or_init(|| Font::parse(epaint_default_fonts::UBUNTU_LIGHT)).as_ref()
}

static FALLBACK: OnceLock<Option<Font<'static>>> = OnceLock::new();

/// Sets a second embedded font (F6) for characters the bundled font lacks, e.g. a
/// system font with Chinese, Japanese or Korean glyphs (`.ttf` or the first font of a
/// `.ttc`). Only fonts with TrueType outlines are taken; it can be set once. Returns
/// whether the font is used.
pub fn set_fallback_font(data: Vec<u8>) -> bool {
    let data: &'static [u8] = Box::leak(data.into_boxed_slice());
    let font = Font::parse_collection(data, 0).filter(|f| f.has_outlines());
    let ok = font.is_some();
    ok && FALLBACK.set(font).is_ok()
}

fn fallback_font() -> Option<&'static Font<'static>> {
    FALLBACK.get().and_then(|f| f.as_ref())
}

/// Number of the system fallback font (`/F6`); the bundled CJK fonts follow (F7 …).
const FALLBACK_SLOT: u8 = 6;

/// The bundled Chinese, Japanese and Korean fonts (F7, F8, F9).
fn cjk_fonts() -> &'static [Option<Font<'static>>] {
    static FONTS: OnceLock<Vec<Option<Font<'static>>>> = OnceLock::new();
    FONTS.get_or_init(|| openecraft_fonts::CJK.iter().map(|f| Font::parse(f.data)).collect())
}

/// The fonts for characters the bundled Ubuntu font lacks with their numbers, in the
/// order they are tried (`openecraft_fonts::fallback_order`, as on screen).
fn fallbacks() -> Vec<(u8, &'static Font<'static>)> {
    let system = fallback_font().map(|f| (FALLBACK_SLOT, f));
    openecraft_fonts::fallback_order(openecraft_i18n::lang().code(), system)
        .into_iter()
        .filter_map(|f| match f {
            Fallback::System(s) => Some(s),
            Fallback::Bundled(b) => {
                let i = openecraft_fonts::CJK.iter().position(|c| c == b)?;
                Some((FALLBACK_SLOT + 1 + i as u8, cjk_fonts()[i].as_ref()?))
            }
        })
        .collect()
}

/// The font a character of a Unicode text is written with: the bundled one (5), or the
/// first fallback that has the glyph.
fn font_of(primary: &'static Font<'static>, c: char, fallbacks: &[(u8, &'static Font<'static>)]) -> (u8, &'static Font<'static>) {
    if primary.glyph(c) != 0 {
        return (5, primary);
    }
    fallbacks.iter().find(|(_, f)| f.glyph(c) != 0).copied().unwrap_or((5, primary))
}

/// The text has characters Helvetica (WinAnsi) cannot show.
fn needs_unicode(text: &str) -> bool {
    text.chars().any(|c| c != '?' && win_ansi(c) == b'?')
}

/// ASCII85 encoding (PDF filter `ASCII85Decode`), with the end marker.
fn ascii85(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len() * 5 / 4 + 4);
    for chunk in data.chunks(4) {
        let mut b = [0u8; 4];
        b[..chunk.len()].copy_from_slice(chunk);
        let v = u32::from_be_bytes(b);
        if v == 0 && chunk.len() == 4 {
            out.push('z');
            continue;
        }
        let mut digits = [0u8; 5];
        let mut n = v;
        for d in digits.iter_mut().rev() {
            *d = (n % 85) as u8 + b'!';
            n /= 85;
        }
        for d in &digits[..chunk.len() + 1] {
            out.push(*d as char);
        }
    }
    out.push_str("~>");
    out
}

/// Points per millimetre.
const K: f32 = 72.0 / 25.4;

/// `application/pdf` → `application#2Fpdf` (PDF name syntax).
fn pdf_name(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c.to_string() } else { format!("#{:02X}", c as u32) }).collect()
}

/// Glyph widths of Helvetica (1/1000 em) for ASCII 32..=126.
const HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556, 556, 556, 556, 556,
    278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667,
    611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833,
    556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];

/// WinAnsi code of a character (`?` if it has none).
fn win_ansi(c: char) -> u8 {
    match c {
        ' '..='~' => c as u8,
        '€' => 0x80,
        '…' => 0x85,
        '–' => 0x96,
        '—' => 0x97,
        '‘' => 0x91,
        '’' => 0x92,
        '“' => 0x93,
        '”' => 0x94,
        '•' => 0x95,
        '\u{a0}'..='\u{ff}' => c as u8,
        _ => b'?',
    }
}

/// PDF string literal content.
fn pdf_string(text: &str) -> String {
    let mut out = String::new();
    for b in text.chars().map(win_ansi) {
        match b {
            b'(' | b')' | b'\\' => {
                out.push('\\');
                out.push(b as char);
            }
            32..=126 => out.push(b as char),
            _ => {
                let _ = write!(out, "\\{b:03o}");
            }
        }
    }
    out
}

fn text_width(text: &str, size: f32) -> f32 {
    text.chars()
        .map(|c| match win_ansi(c) {
            b @ 32..=126 => HELVETICA[(b - 32) as usize] as f32,
            _ => 556.0,
        })
        .sum::<f32>()
        * size
        / 1000.0
}

fn rgb(ink: Ink) -> (f32, f32, f32) {
    match ink {
        Ink::Frame | Ink::FrameText => (0.33, 0.33, 0.33),
        Ink::CrossRef => (0.1, 0.24, 0.56),
        Ink::Error => (0.8, 0.0, 0.0),
        Ink::Paper => (1.0, 1.0, 1.0),
        Ink::Plate => (0.95, 0.95, 0.95),
        Ink::Fill => (0.86, 0.86, 0.86),
        Ink::Color([r, g, b]) => (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0),
        _ => (0.0, 0.0, 0.0),
    }
}

fn x(v: f32) -> f32 {
    v * K
}


/// A bitmap as PDF image: dictionary entries and the data as hex.
struct PdfImage {
    dict: String,
    hex: String,
}

fn hex(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len() * 2 + 1);
    for b in data {
        let _ = write!(out, "{b:02X}");
    }
    out.push('>');
    out
}

/// JPEG files (grey or RGB) go into the PDF as they are; PNG files without
/// transparency, palette or interlacing use the PNG predictor of the PDF.
fn pdf_image(bytes: &[u8]) -> Option<PdfImage> {
    if bytes.starts_with(&[0xFF, 0xD8]) {
        let mut i = 2;
        while i + 9 < bytes.len() {
            if bytes[i] != 0xFF {
                i += 1;
                continue;
            }
            let marker = bytes[i + 1];
            let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
            if (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
                let h = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]);
                let w = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]);
                let space = match bytes[i + 9] {
                    1 => "DeviceGray",
                    3 => "DeviceRGB",
                    _ => return None,
                };
                return Some(PdfImage {
                    dict: format!("/Width {w} /Height {h} /ColorSpace /{space} /BitsPerComponent 8 /Filter [/ASCIIHexDecode /DCTDecode]"),
                    hex: hex(bytes),
                });
            }
            i += 2 + len;
        }
        return None;
    }
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return None;
    }
    let mut i = 8;
    let (mut w, mut h, mut colors) = (0, 0, 0);
    let mut idat = Vec::new();
    while i + 8 <= bytes.len() {
        let len = u32::from_be_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]) as usize;
        let kind = &bytes[i + 4..i + 8];
        let data = bytes.get(i + 8..i + 8 + len)?;
        match kind {
            b"IHDR" if len >= 13 => {
                w = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
                h = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
                let (depth, color, interlace) = (data[8], data[9], data[12]);
                colors = match (depth, color, interlace) {
                    (8, 0, 0) => 1,
                    (8, 2, 0) => 3,
                    _ => return None,
                };
            }
            b"IDAT" => idat.extend_from_slice(data),
            b"IEND" => break,
            _ => {}
        }
        i += 12 + len;
    }
    if colors == 0 || idat.is_empty() {
        return None;
    }
    let space = if colors == 1 { "DeviceGray" } else { "DeviceRGB" };
    Some(PdfImage {
        dict: format!(
            "/Width {w} /Height {h} /ColorSpace /{space} /BitsPerComponent 8 /Filter [/ASCIIHexDecode /FlateDecode] /DecodeParms [null << /Predictor 15 /Colors {colors} /BitsPerComponent 8 /Columns {w} >>]"
        ),
        hex: hex(&idat),
    })
}

/// Collects the content stream and the links of one page.
#[derive(Default)]
pub struct PdfDraw {
    /// Paper height in mm (PDF y runs upwards).
    h: f32,
    ops: String,
    /// (x0, y0, x1, y1 in PDF units, target page index)
    links: Vec<([f32; 4], usize)>,
    /// (rectangle in PDF units, URL) of the hyperlinks.
    urls: Vec<([f32; 4], String)>,
    /// Bitmaps used by the page (`/Im0`, `/Im1` …).
    images: Vec<PdfImage>,
    /// Glyphs of the embedded font used on the page, with their characters.
    glyphs: BTreeMap<u16, char>,
    /// Glyphs of the fallback fonts (F6 …) used on the page, by font number.
    fallback_glyphs: BTreeMap<u8, BTreeMap<u16, char>>,
}

impl PdfDraw {
    fn y(&self, v: f32) -> f32 {
        (self.h - v) * K
    }

    fn stroke(&mut self, width: f32, ink: Ink, dashed: bool) {
        let (r, g, b) = rgb(ink);
        let dash = if dashed { "[4.25 2.83] 0 d" } else { "[] 0 d" };
        let _ = write!(self.ops, "{r:.3} {g:.3} {b:.3} RG {:.3} w {dash} ", width.max(0.1) * K);
    }

    fn fill(&mut self, ink: Ink) {
        let (r, g, b) = rgb(ink);
        let _ = write!(self.ops, "{r:.3} {g:.3} {b:.3} rg ");
    }

    fn path(&mut self, pts: &[P]) {
        for (i, q) in pts.iter().enumerate() {
            let _ = write!(self.ops, "{:.2} {:.2} {} ", x(q.x), self.y(q.y), if i == 0 { "m" } else { "l" });
        }
    }
}

impl Draw for PdfDraw {
    fn line(&mut self, a: P, b: P, width: f32, ink: Ink, dashed: bool) {
        self.stroke(width, ink, dashed);
        self.path(&[a, b]);
        self.ops.push_str("S\n");
    }

    fn poly(&mut self, pts: &[P], closed: bool, width: f32, ink: Ink, fill: bool) {
        if pts.len() < 2 {
            return;
        }
        if fill {
            self.fill(ink);
        }
        if !fill || width > 0.0 {
            self.stroke(width, ink, false);
        }
        self.path(pts);
        let op = match (fill, width > 0.0, closed) {
            (true, true, _) => "b",
            (true, false, _) => "f",
            (false, _, true) => "s",
            (false, _, false) => "S",
        };
        self.ops.push_str(op);
        self.ops.push('\n');
    }

    fn circle(&mut self, c: P, r: f32, width: f32, ink: Ink, fill: bool) {
        // Four Bézier arcs.
        let (cx, cy, r) = (x(c.x), self.y(c.y), r * K);
        let k = 0.552_284_8 * r;
        if fill {
            self.fill(ink);
        } else {
            self.stroke(width, ink, false);
        }
        let _ = writeln!(
            self.ops,
            "{:.2} {:.2} m {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c {}",
            cx + r, cy,
            cx + r, cy + k, cx + k, cy + r, cx, cy + r,
            cx - k, cy + r, cx - r, cy + k, cx - r, cy,
            cx - r, cy - k, cx - k, cy - r, cx, cy - r,
            cx + k, cy - r, cx + r, cy - k, cx + r, cy,
            if fill { "f" } else { "s" }
        );
    }

    fn text(&mut self, pos: P, text: &str, size: f32, h: Anchor, v: VAnchor, ink: Ink) {
        self.styled_text(pos, text, &TextStyle::plain(size, h, v), ink);
    }

    fn styled_text(&mut self, pos: P, text: &str, st: &TextStyle, ink: Ink) {
        if text.is_empty() {
            return;
        }
        // Same metrics as the SVG export.
        let fs = st.size * 1.25 * K;
        let font = unicode_font().filter(|_| needs_unicode(text));
        let fallbacks = if font.is_some() { fallbacks() } else { Vec::new() };
        let w = match font {
            Some(f) => {
                text.chars()
                    .map(|c| {
                        let (_, ff) = font_of(f, c, &fallbacks);
                        ff.advance(ff.glyph(c)) as f32 / ff.units_per_em as f32
                    })
                    .sum::<f32>()
                    * fs
                    / K
            }
            None => text_width(text, fs) / K,
        };
        let dx = match st.h {
            Anchor::Left => 0.0,
            Anchor::Center => -w / 2.0,
            Anchor::Right => -w,
        };
        let dy = match st.v {
            VAnchor::Top => st.size * 0.95,
            VAnchor::Middle => st.size * 0.35,
            VAnchor::Bottom => -st.size * 0.1,
        };
        // Start of the baseline, turned with the text around its anchor.
        let start = pos + crate::text::rotate(p(dx, dy), st.angle);
        let face = match (st.bold, st.italic) {
            (false, false) => 1,
            (true, false) => 2,
            (false, true) => 3,
            (true, true) => 4,
        };
        self.fill(ink);
        if let Some(f) = font {
            // Runs of the bundled font (F5) and of the fallback fonts (F6 …).
            let mut runs: Vec<(u8, String)> = Vec::new();
            for c in text.chars() {
                let (slot, ff) = font_of(f, c, &fallbacks);
                let g = ff.glyph(c);
                if slot == 5 {
                    self.glyphs.entry(g).or_insert(c);
                } else {
                    self.fallback_glyphs.entry(slot).or_default().entry(g).or_insert(c);
                }
                match runs.last_mut() {
                    Some((s, hex)) if *s == slot => {
                        let _ = write!(hex, "{g:04X}");
                    }
                    _ => runs.push((slot, format!("{g:04X}"))),
                }
            }
            let gids: String = runs.iter().map(|(slot, hex)| format!("/F{slot} {fs:.2} Tf <{hex}> Tj ")).collect();
            // Bold: filled and stroked outlines; italic: slanted text space.
            let (s, c) = st.angle.to_radians().sin_cos();
            let k = if st.italic { 0.2 } else { 0.0 };
            let (r, g, b) = rgb(ink);
            let bold = if st.bold { format!("{r:.3} {g:.3} {b:.3} RG {:.3} w 2 Tr ", fs * 0.03) } else { String::new() };
            let _ = writeln!(
                self.ops,
                "BT {bold}{c:.4} {s:.4} {:.4} {:.4} {:.2} {:.2} Tm {gids}{}ET",
                k * c - s,
                k * s + c,
                x(start.x),
                self.y(start.y),
                if st.bold { "0 Tr " } else { "" }
            );
        } else if st.angle == 0.0 {
            let _ = writeln!(self.ops, "BT /F{face} {fs:.2} Tf {:.2} {:.2} Td ({}) Tj ET", x(start.x), self.y(start.y), pdf_string(text));
        } else {
            let (s, c) = st.angle.to_radians().sin_cos();
            let _ = writeln!(self.ops, "BT /F{face} {fs:.2} Tf {c:.4} {s:.4} {:.4} {c:.4} {:.2} {:.2} Tm ({}) Tj ET", -s, x(start.x), self.y(start.y), pdf_string(text));
        }
        if st.underline {
            let a = start + crate::text::rotate(p(0.0, st.size * 0.15), st.angle);
            self.line(a, a + crate::text::rotate(p(w, 0.0), st.angle), st.size * 0.06, ink, false);
        }
    }

    fn link(&mut self, min: P, max: P, page: usize) {
        self.links.push(([x(min.x), self.y(max.y), x(max.x), self.y(min.y)], page));
    }

    fn link_url(&mut self, min: P, max: P, url: &str) {
        self.urls.push(([x(min.x), self.y(max.y), x(max.x), self.y(min.y)], url.to_string()));
    }

    fn image_quad(&mut self, img: &crate::model::ImageItem, corners: &[P; 4]) {
        let Some(pi) = img.file.bytes().ok().and_then(|b| pdf_image(&b)) else {
            crate::render::image_placeholder(self, img);
            return;
        };
        let u = corners[2] - corners[3];
        let v = corners[0] - corners[3];
        let n = self.images.len();
        self.images.push(pi);
        let _ = writeln!(self.ops, "q {:.6} {:.6} {:.6} {:.6} {:.6} {:.6} cm /Im{n} Do Q",
            u.x * K, -u.y * K, v.x * K, -v.y * K, x(corners[3].x), self.y(corners[3].y));
    }

    fn image(&mut self, img: &crate::model::ImageItem) {
        let Some(pi) = img.file.bytes().ok().and_then(|b| pdf_image(&b)) else {
            crate::render::image_placeholder(self, img);
            return;
        };
        let r = crate::geom::R::from_points(img.a, img.b);
        let n = self.images.len();
        self.images.push(pi);
        let _ = writeln!(self.ops, "q {:.2} 0 0 {:.2} {:.2} {:.2} cm /Im{n} Do Q", r.width() * K, r.height() * K, x(r.min.x), self.y(r.max.y));
    }
}

/// Builds the PDF objects and the cross reference table.
struct Writer {
    out: String,
    offsets: Vec<usize>,
}

impl Writer {
    fn obj(&mut self, n: usize, body: &str) {
        if self.offsets.len() <= n {
            self.offsets.resize(n + 1, 0);
        }
        self.offsets[n] = self.out.len();
        let _ = write!(self.out, "{n} 0 obj\n{body}\nendobj\n");
    }
}

/// Writes the embedded font as objects `obj` … `obj + 4`: Type0 font, CID font, font
/// descriptor, font file (subset, compressed) and ToUnicode map.
/// Embeds a font with the glyphs used (five objects from `next`); `None` without font
/// or glyphs.
fn embed_font(w: &mut Writer, next: &mut usize, font: Option<&Font>, glyphs: &BTreeMap<u16, char>, name: &str) -> Option<usize> {
    let f = font.filter(|_| !glyphs.is_empty())?;
    let obj = *next;
    *next += 5;
    write_unicode_font(w, obj, f, glyphs, name);
    Some(obj)
}

/// Embeds the fallback fonts the pages use; returns their font resources (`/F6 n 0 R` …).
fn embed_fallback_fonts(w: &mut Writer, next: &mut usize, pages: &[PdfDraw]) -> String {
    let mut fallback_glyphs: BTreeMap<u8, BTreeMap<u16, char>> = BTreeMap::new();
    for d in pages {
        for (slot, glyphs) in &d.fallback_glyphs {
            fallback_glyphs.entry(*slot).or_default().extend(glyphs.iter().map(|(g, c)| (*g, *c)));
        }
    }
    let mut refs = String::new();
    for (slot, glyphs) in &fallback_glyphs {
        let (font, name) = match slot.checked_sub(FALLBACK_SLOT + 1) {
            None => (fallback_font(), "OECFBK+Fallback".to_string()),
            Some(i) => (cjk_fonts()[i as usize].as_ref(), format!("OECCJK+{}", openecraft_fonts::CJK[i as usize].name)),
        };
        if let Some(obj) = embed_font(w, next, font, glyphs, &name) {
            let _ = write!(refs, " /F{slot} {obj} 0 R");
        }
    }
    refs
}

fn write_unicode_font(w: &mut Writer, obj: usize, f: &Font, glyphs: &BTreeMap<u16, char>, name: &str) {
    let name = name.replace(' ', "");
    let name = name.as_str();
    let scale = 1000.0 / f.units_per_em as f32;
    let (cid, desc, file, map) = (obj + 1, obj + 2, obj + 3, obj + 4);
    w.obj(obj, &format!("<< /Type /Font /Subtype /Type0 /BaseFont /{name} /Encoding /Identity-H /DescendantFonts [{cid} 0 R] /ToUnicode {map} 0 R >>"));
    let widths: Vec<String> = glyphs.keys().map(|g| format!("{g} [{:.0}]", f.advance(*g) as f32 * scale)).collect();
    w.obj(
        cid,
        &format!(
            "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{name} /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor {desc} 0 R /DW 500 /W [{}] /CIDToGIDMap /Identity >>",
            widths.join(" ")
        ),
    );
    let b = f.bbox.map(|v| (v as f32 * scale).round() as i32);
    w.obj(
        desc,
        &format!(
            "<< /Type /FontDescriptor /FontName /{name} /Flags 32 /FontBBox [{} {} {} {}] /ItalicAngle 0 /Ascent {:.0} /Descent {:.0} /CapHeight {:.0} /StemV 70 /FontFile2 {file} 0 R >>",
            b[0],
            b[1],
            b[2],
            b[3],
            f.ascent as f32 * scale,
            f.descent as f32 * scale,
            f.cap_height as f32 * scale
        ),
    );
    let subset = f.subset(&glyphs.keys().copied().collect());
    let data = ascii85(&miniz_oxide::deflate::compress_to_vec_zlib(&subset, 9));
    w.obj(file, &format!("<< /Length {} /Length1 {} /Filter [/ASCII85Decode /FlateDecode] >>\nstream\n{data}\nendstream", data.len() + 1, subset.len()));
    let mut cmap = String::from("/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def /CMapName /OEC-UCS def /CMapType 2 def\n1 begincodespacerange <0000> <FFFF> endcodespacerange\n");
    let entries: Vec<(&u16, &char)> = glyphs.iter().collect();
    for chunk in entries.chunks(100) {
        let _ = writeln!(cmap, "{} beginbfchar", chunk.len());
        for (g, c) in chunk {
            let mut units = [0u16; 2];
            let hex: String = c.encode_utf16(&mut units).iter().map(|u| format!("{u:04X}")).collect();
            let _ = writeln!(cmap, "<{g:04X}> <{hex}>");
        }
        cmap.push_str("endbfchar\n");
    }
    cmap.push_str("endcmap CMapName currentdict /CMap defineresource pop end end\n");
    w.obj(map, &format!("<< /Length {} >>\nstream\n{cmap}endstream", cmap.len()));
}

/// All pages of the project as one PDF document.
pub fn document(prj: &Project) -> String {
    document_with(prj, &Analysis::with_builtin(prj))
}

/// All pages as PDF, drawn with the host's analysis (its reports, its index).
pub fn document_with(prj: &Project, an: &Analysis) -> String {
    document_pages(prj, an, &(0..prj.pages.len()).collect::<Vec<_>>())
}

/// The PDF of the pages `sel` (indices in project order); links to pages left out are
/// dropped.
pub fn document_pages(prj: &Project, an: &Analysis, sel: &[usize]) -> String {
    let idx = &an.index;
    let pages: Vec<PdfDraw> = sel
        .iter()
        .map(|&i| {
            let mut d = PdfDraw { h: crate::frame::of_page(prj, i).height(), ..Default::default() };
            draw_page(&mut d, prj, idx, an, i, &Opts::default());
            d
        })
        .collect();

    // Object numbers: 1 catalog, 2 page tree, 3 font, 4 outline root, 5 info, then
    // per page: page, content, bookmark; then the link annotations.
    let n = pages.len();
    let page_obj = |i: usize| 6 + 3 * i;
    let mut next = 6 + 3 * n;
    let mut w = Writer { out: String::from("%PDF-1.4\n"), offsets: vec![0] };
    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", page_obj(i))).collect();
    w.obj(2, &format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")));
    w.obj(3, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>");
    // The embedded font (F5) with the glyphs all pages use.
    let glyphs: BTreeMap<u16, char> = pages.iter().flat_map(|d| d.glyphs.iter().map(|(g, c)| (*g, *c))).collect();
    let unicode = embed_font(&mut w, &mut next, unicode_font(), &glyphs, "OECUBU+Ubuntu-Light");
    // The fallback fonts (F6 …) for glyphs the bundled font lacks.
    let fallback_refs = embed_fallback_fonts(&mut w, &mut next, &pages);
    // Bold, italic and bold italic texts (F2 … F4).
    let fonts = [next, next + 1, next + 2];
    next += 3;
    for (k, name) in ["Helvetica-Bold", "Helvetica-Oblique", "Helvetica-BoldOblique"].into_iter().enumerate() {
        w.obj(fonts[k], &format!("<< /Type /Font /Subtype /Type1 /BaseFont /{name} /Encoding /WinAnsiEncoding >>"));
    }
    if n > 0 {
        w.obj(4, &format!("<< /Type /Outlines /First {} 0 R /Last {} 0 R /Count {n} >>", page_obj(0) + 2, page_obj(n - 1) + 2));
    } else {
        w.obj(4, "<< /Type /Outlines /Count 0 >>");
    }
    w.obj(5, &format!("<< /Title ({}) /Producer (OpenECraft) >>", pdf_string(&prj.info.name)));
    let mut attachments = Vec::new();
    for (k, d) in pages.iter().enumerate() {
        let i = sel[k];
        let frame = crate::frame::of_page(prj, i);
        let (width, height) = (frame.width() * K, frame.height() * K);
        let mut annots = Vec::new();
        let pg = &prj.pages[i];
        if let (PageKind::Document, Some(doc)) = (pg.kind, &pg.document) {
            if let Ok(bytes) = doc.bytes() {
                let (min, max) = crate::document::icon_area(&frame);
                let (file, spec, annot) = (next, next + 1, next + 2);
                next += 3;
                let hex = hex(&bytes);
                w.obj(
                    file,
                    &format!(
                        "<< /Type /EmbeddedFile /Subtype /{} /Filter /ASCIIHexDecode /Length {} /Params << /Size {} >> >>\nstream\n{hex}\nendstream",
                        pdf_name(doc.mime()),
                        hex.len(),
                        bytes.len()
                    ),
                );
                let name = pdf_string(&doc.name);
                w.obj(spec, &format!("<< /Type /Filespec /F ({name}) /UF ({name}) /EF << /F {file} 0 R >> >>"));
                w.obj(
                    annot,
                    &format!(
                        "<< /Type /Annot /Subtype /FileAttachment /Rect [{:.2} {:.2} {:.2} {:.2}] /FS {spec} 0 R /Contents ({name}) /Name /Paperclip >>",
                        min.x * K,
                        (frame.height() - max.y) * K,
                        max.x * K,
                        (frame.height() - min.y) * K
                    ),
                );
                annots.push(format!("{annot} 0 R"));
                attachments.push(format!("({name}) {spec} 0 R"));
            }
        }
        for (rect, target) in &d.links {
            let Some(target) = sel.iter().position(|x| x == target).filter(|t| *t != k) else { continue };
            let body = format!(
                "<< /Type /Annot /Subtype /Link /Rect [{:.2} {:.2} {:.2} {:.2}] /Border [0 0 0] /Dest [{} 0 R /Fit] >>",
                rect[0], rect[1], rect[2], rect[3], page_obj(target)
            );
            w.obj(next, &body);
            annots.push(format!("{next} 0 R"));
            next += 1;
        }
        for (rect, url) in &d.urls {
            let body = format!(
                "<< /Type /Annot /Subtype /Link /Rect [{:.2} {:.2} {:.2} {:.2}] /Border [0 0 0] /A << /S /URI /URI ({}) >> >>",
                rect[0],
                rect[1],
                rect[2],
                rect[3],
                pdf_string(url)
            );
            w.obj(next, &body);
            annots.push(format!("{next} 0 R"));
            next += 1;
        }
        let mut xobjects = Vec::new();
        for (k, img) in d.images.iter().enumerate() {
            w.obj(
                next,
                &format!("<< /Type /XObject /Subtype /Image {} /Length {} >>\nstream\n{}\nendstream", img.dict, img.hex.len(), img.hex),
            );
            xobjects.push(format!("/Im{k} {next} 0 R"));
            next += 1;
        }
        let xobject = if xobjects.is_empty() { String::new() } else { format!(" /XObject << {} >>", xobjects.join(" ")) };
        let po = page_obj(k);
        w.obj(
            po,
            &format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width:.2} {height:.2}] /Resources << /Font << /F1 3 0 R /F2 {} 0 R /F3 {} 0 R /F4 {} 0 R{}{fallback_refs} >>{xobject} >> /Contents {} 0 R /Annots [{}] >>",
                fonts[0],
                fonts[1],
                fonts[2],
                unicode.map(|u| format!(" /F5 {u} 0 R")).unwrap_or_default(),
                po + 1,
                annots.join(" ")
            ),
        );
        w.obj(po + 1, &format!("<< /Length {} >>\nstream\n{}endstream", d.ops.len(), d.ops));
        let title = format!("{} {}", crate::structure::page_full_name(pg), pg.description).trim().to_string();
        let mut bookmark = format!("<< /Title ({}) /Parent 4 0 R /Dest [{po} 0 R /Fit]", pdf_string(&title));
        if k > 0 {
            let _ = write!(bookmark, " /Prev {} 0 R", page_obj(k - 1) + 2);
        }
        if k + 1 < n {
            let _ = write!(bookmark, " /Next {} 0 R", page_obj(k + 1) + 2);
        }
        bookmark.push_str(" >>");
        w.obj(po + 2, &bookmark);
    }
    let names = if attachments.is_empty() { String::new() } else { format!(" /Names << /EmbeddedFiles << /Names [{}] >> >>", attachments.join(" ")) };
    w.obj(1, &format!("<< /Type /Catalog /Pages 2 0 R /Outlines 4 0 R /PageMode /UseOutlines{names} >>"));
    let xref = w.out.len();
    let count = w.offsets.len();
    let _ = writeln!(w.out, "xref\n0 {count}\n0000000000 65535 f ");
    for off in &w.offsets[1..] {
        let _ = writeln!(w.out, "{off:010} 00000 n ");
    }
    let _ = write!(w.out, "trailer\n<< /Size {count} /Root 1 0 R /Info 5 0 R >>\nstartxref\n{xref}\n%%EOF\n");
    w.out
}

#[cfg(test)]
mod tests;
