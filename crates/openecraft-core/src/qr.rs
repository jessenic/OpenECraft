//! QR codes (ISO/IEC 18004): byte mode, error correction level M, the smallest of
//! the versions 1 to 40 that holds the text, the mask with the lowest penalty. Used
//! by the QR code elements of title blocks and drawn forms.

/// Error correction codewords per block (level M) by version.
const ECC_PER_BLOCK: [usize; 41] = [
    0, 10, 16, 26, 18, 24, 16, 18, 22, 22, 26, 30, 22, 22, 24, 24, 28, 28, 26, 26, 26, 26, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28,
    28, 28, 28, 28, 28,
];

/// Error correction blocks (level M) by version.
const BLOCKS: [usize; 41] = [
    0, 1, 1, 1, 2, 2, 4, 4, 4, 5, 5, 5, 8, 9, 9, 10, 10, 11, 13, 14, 16, 17, 17, 18, 20, 21, 23, 25, 26, 28, 29, 31, 33, 35, 37, 38, 40, 43, 45,
    47, 49,
];

/// Format bits of level M.
const LEVEL_M: u32 = 0;

/// A QR code: `size` × `size` modules, true for dark ones.
#[derive(Clone, Debug, PartialEq)]
pub struct Qr {
    pub version: usize,
    pub size: usize,
    pub mask: u8,
    modules: Vec<bool>,
    function: Vec<bool>,
}

impl Qr {
    /// The code of `text`; `None` when it is longer than version 40 holds (2331 bytes).
    pub fn encode(text: &str) -> Option<Qr> {
        Qr::encode_with_mask(text.as_bytes(), None)
    }

    /// The code with mask `mask` (0 … 7) or the best one.
    pub fn encode_with_mask(data: &[u8], mask: Option<u8>) -> Option<Qr> {
        let version = (1..=40).find(|&v| 4 + count_bits(v) + data.len() * 8 <= data_codewords(v) * 8)?;
        let capacity = data_codewords(version) * 8;
        let mut bits: Vec<bool> = Vec::with_capacity(capacity);
        push_bits(&mut bits, 0b0100, 4);
        push_bits(&mut bits, data.len() as u32, count_bits(version));
        for b in data {
            push_bits(&mut bits, u32::from(*b), 8);
        }
        let terminator = (capacity - bits.len()).min(4);
        push_bits(&mut bits, 0, terminator);
        let pad = (8 - bits.len() % 8) % 8;
        push_bits(&mut bits, 0, pad);
        let mut bytes: Vec<u8> = bits.chunks(8).map(|c| c.iter().fold(0u8, |acc, b| acc << 1 | u8::from(*b))).collect();
        for pad in [0xEC, 0x11].into_iter().cycle() {
            if bytes.len() >= capacity / 8 {
                break;
            }
            bytes.push(pad);
        }
        let size = version * 4 + 17;
        let mut qr = Qr { version, size, mask: 0, modules: vec![false; size * size], function: vec![false; size * size] };
        qr.draw_function_patterns();
        qr.draw_codewords(&add_ecc_and_interleave(&bytes, version));
        let mask = mask.unwrap_or_else(|| {
            (0..8u8)
                .min_by_key(|&m| {
                    qr.apply_mask(m);
                    qr.draw_format_bits(m);
                    let p = qr.penalty();
                    qr.apply_mask(m);
                    p
                })
                .unwrap_or(0)
        });
        qr.apply_mask(mask);
        qr.draw_format_bits(mask);
        qr.mask = mask;
        Some(qr)
    }

    /// Whether the module in column `x`, row `y` is dark.
    pub fn dark(&self, x: usize, y: usize) -> bool {
        x < self.size && y < self.size && self.modules[y * self.size + x]
    }

    fn set_function(&mut self, x: usize, y: usize, dark: bool) {
        self.modules[y * self.size + x] = dark;
        self.function[y * self.size + x] = true;
    }

    fn draw_function_patterns(&mut self) {
        let size = self.size;
        for i in 0..size {
            self.set_function(6, i, i % 2 == 0);
            self.set_function(i, 6, i % 2 == 0);
        }
        for (x, y) in [(3, 3), (size - 4, 3), (3, size - 4)] {
            for dy in -4i32..=4 {
                for dx in -4i32..=4 {
                    let (xx, yy) = (x as i32 + dx, y as i32 + dy);
                    if (0..size as i32).contains(&xx) && (0..size as i32).contains(&yy) {
                        let dist = dx.abs().max(dy.abs());
                        self.set_function(xx as usize, yy as usize, dist != 2 && dist != 4);
                    }
                }
            }
        }
        let pos = alignment_positions(self.version);
        let n = pos.len();
        for i in 0..n {
            for j in 0..n {
                if (i == 0 && j == 0) || (i == 0 && j == n - 1) || (i == n - 1 && j == 0) {
                    continue;
                }
                for dy in -2i32..=2 {
                    for dx in -2i32..=2 {
                        self.set_function((pos[i] as i32 + dx) as usize, (pos[j] as i32 + dy) as usize, dx.abs().max(dy.abs()) != 1);
                    }
                }
            }
        }
        self.draw_format_bits(0);
        self.draw_version();
    }

    fn draw_format_bits(&mut self, mask: u8) {
        let data = LEVEL_M << 3 | u32::from(mask);
        let mut rem = data;
        for _ in 0..10 {
            rem = (rem << 1) ^ ((rem >> 9) * 0x537);
        }
        let bits = (data << 10 | rem) ^ 0x5412;
        let bit = |i: usize| (bits >> i) & 1 != 0;
        let size = self.size;
        for i in 0..=5 {
            self.set_function(8, i, bit(i));
        }
        self.set_function(8, 7, bit(6));
        self.set_function(8, 8, bit(7));
        self.set_function(7, 8, bit(8));
        for i in 9..15 {
            self.set_function(14 - i, 8, bit(i));
        }
        for i in 0..8 {
            self.set_function(size - 1 - i, 8, bit(i));
        }
        for i in 8..15 {
            self.set_function(8, size - 15 + i, bit(i));
        }
        self.set_function(8, size - 8, true);
    }

    fn draw_version(&mut self) {
        if self.version < 7 {
            return;
        }
        let mut rem = self.version as u32;
        for _ in 0..12 {
            rem = (rem << 1) ^ ((rem >> 11) * 0x1F25);
        }
        let bits = (self.version as u32) << 12 | rem;
        for i in 0..18 {
            let dark = (bits >> i) & 1 != 0;
            let (a, b) = (self.size - 11 + i % 3, i / 3);
            self.set_function(a, b, dark);
            self.set_function(b, a, dark);
        }
    }

    fn draw_codewords(&mut self, data: &[u8]) {
        let size = self.size;
        let mut i = 0;
        let mut right = size - 1;
        while right >= 1 {
            if right == 6 {
                right = 5;
            }
            for vert in 0..size {
                for j in 0..2 {
                    let x = right - j;
                    let upward = (right + 1) & 2 == 0;
                    let y = if upward { size - 1 - vert } else { vert };
                    if !self.function[y * size + x] && i < data.len() * 8 {
                        self.modules[y * size + x] = (data[i >> 3] >> (7 - (i & 7))) & 1 != 0;
                        i += 1;
                    }
                }
            }
            if right < 2 {
                break;
            }
            right -= 2;
        }
    }

    fn apply_mask(&mut self, mask: u8) {
        let size = self.size;
        for y in 0..size {
            for x in 0..size {
                let invert = match mask {
                    0 => (x + y) % 2 == 0,
                    1 => y % 2 == 0,
                    2 => x % 3 == 0,
                    3 => (x + y) % 3 == 0,
                    4 => (x / 3 + y / 2) % 2 == 0,
                    5 => x * y % 2 + x * y % 3 == 0,
                    6 => (x * y % 2 + x * y % 3) % 2 == 0,
                    _ => ((x + y) % 2 + x * y % 3) % 2 == 0,
                };
                let i = y * size + x;
                self.modules[i] ^= invert && !self.function[i];
            }
        }
    }

    fn penalty(&self) -> usize {
        let size = self.size;
        let mut result = 0;
        for transpose in [false, true] {
            for a in 0..size {
                let (mut color, mut run, mut history) = (false, 0usize, [0usize; 7]);
                for b in 0..size {
                    let dark = if transpose { self.dark(a, b) } else { self.dark(b, a) };
                    if dark == color {
                        run += 1;
                        if run == 5 {
                            result += 3;
                        } else if run > 5 {
                            result += 1;
                        }
                    } else {
                        add_history(run, &mut history, size);
                        if !color {
                            result += finder_patterns(&history) * 40;
                        }
                        color = dark;
                        run = 1;
                    }
                }
                if color {
                    add_history(run, &mut history, size);
                    run = 0;
                }
                add_history(run + size, &mut history, size);
                result += finder_patterns(&history) * 40;
            }
        }
        for y in 0..size - 1 {
            for x in 0..size - 1 {
                let c = self.dark(x, y);
                if c == self.dark(x + 1, y) && c == self.dark(x, y + 1) && c == self.dark(x + 1, y + 1) {
                    result += 3;
                }
            }
        }
        let dark = self.modules.iter().filter(|m| **m).count() as i64;
        let total = (size * size) as i64;
        let k = ((dark * 20 - total * 10).abs() + total - 1) / total - 1;
        result + k as usize * 10
    }
}

fn add_history(mut run: usize, history: &mut [usize; 7], size: usize) {
    if history[0] == 0 {
        run += size;
    }
    history.copy_within(0..6, 1);
    history[0] = run;
}

fn finder_patterns(h: &[usize; 7]) -> usize {
    let n = h[1];
    let core = n > 0 && h[2] == n && h[3] == n * 3 && h[4] == n && h[5] == n;
    usize::from(core && h[0] >= n * 4 && h[6] >= n) + usize::from(core && h[6] >= n * 4 && h[0] >= n)
}

fn push_bits(bits: &mut Vec<bool>, value: u32, n: usize) {
    bits.extend((0..n).rev().map(|i| (value >> i) & 1 != 0));
}

/// Bits of the character count in byte mode.
fn count_bits(version: usize) -> usize {
    if version < 10 { 8 } else { 16 }
}

/// Modules for data and error correction (without function patterns).
pub fn raw_data_modules(version: usize) -> usize {
    let mut result = (16 * version + 128) * version + 64;
    if version >= 2 {
        let n = version / 7 + 2;
        result -= (25 * n - 10) * n - 55;
        if version >= 7 {
            result -= 36;
        }
    }
    result
}

/// Data codewords of a version at level M.
pub fn data_codewords(version: usize) -> usize {
    raw_data_modules(version) / 8 - ECC_PER_BLOCK[version] * BLOCKS[version]
}

fn alignment_positions(version: usize) -> Vec<usize> {
    if version == 1 {
        return Vec::new();
    }
    let n = version / 7 + 2;
    let step = (version * 8 + n * 3 + 5) / (n * 4 - 4) * 2;
    let size = version * 4 + 17;
    let mut v: Vec<usize> = (0..n - 1).map(|i| size - 7 - i * step).collect();
    v.push(6);
    v.reverse();
    v
}

fn gf_mul(x: u8, y: u8) -> u8 {
    let mut z: u32 = 0;
    for i in (0..8).rev() {
        z = (z << 1) ^ ((z >> 7) * 0x11D);
        z ^= ((u32::from(y) >> i) & 1) * u32::from(x);
    }
    z as u8
}

fn rs_divisor(degree: usize) -> Vec<u8> {
    let mut result = vec![0u8; degree];
    result[degree - 1] = 1;
    let mut root = 1u8;
    for _ in 0..degree {
        for j in 0..degree {
            result[j] = gf_mul(result[j], root);
            if j + 1 < degree {
                result[j] ^= result[j + 1];
            }
        }
        root = gf_mul(root, 0x02);
    }
    result
}

fn rs_remainder(data: &[u8], divisor: &[u8]) -> Vec<u8> {
    let mut result = vec![0u8; divisor.len()];
    for b in data {
        let factor = b ^ result.remove(0);
        result.push(0);
        for (r, c) in result.iter_mut().zip(divisor) {
            *r ^= gf_mul(*c, factor);
        }
    }
    result
}

fn add_ecc_and_interleave(data: &[u8], version: usize) -> Vec<u8> {
    let (blocks, ecc_len) = (BLOCKS[version], ECC_PER_BLOCK[version]);
    let raw = raw_data_modules(version) / 8;
    let short_blocks = blocks - raw % blocks;
    let short_len = raw / blocks;
    let divisor = rs_divisor(ecc_len);
    let mut all: Vec<Vec<u8>> = Vec::with_capacity(blocks);
    let mut k = 0;
    for i in 0..blocks {
        let len = short_len - ecc_len + usize::from(i >= short_blocks);
        let mut block = data[k..k + len].to_vec();
        k += len;
        let ecc = rs_remainder(&block, &divisor);
        if i < short_blocks {
            block.push(0);
        }
        block.extend(ecc);
        all.push(block);
    }
    let mut out = Vec::with_capacity(raw);
    for i in 0..all[0].len() {
        for (j, block) in all.iter().enumerate() {
            if i != short_len - ecc_len || j >= short_blocks {
                out.push(block[i]);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests;
