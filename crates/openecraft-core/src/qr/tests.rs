use super::*;

fn fnv(qr: &Qr) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for y in 0..qr.size {
        for x in 0..qr.size {
            h = (h ^ u64::from(qr.dark(x, y))).wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

fn text(n: usize) -> String {
    let s = "https://openecraft.example/projects/42?page=7&rev=B ";
    s.repeat(n / s.len() + 1)[..n].to_string()
}

#[test]
fn byte_capacities_of_level_m() {
    let bytes = |v: usize| (data_codewords(v) * 8 - 4 - count_bits(v)) / 8;
    assert_eq!([bytes(1), bytes(2), bytes(10), bytes(27), bytes(40)], [14, 26, 213, 1125, 2331]);
    assert!(Qr::encode(&text(2331)).is_some());
    assert!(Qr::encode(&text(2332)).is_none());
}

#[test]
fn small_code_matches_the_reference() {
    // Reference: qrcodegen 1.8 (Nayuki), "OpenECraft", byte mode, level M, mask 2;
    // read back by zxing-cpp.
    let reference = [
        "#######....##.#######",
        "#.....#...##..#.....#",
        "#.###.#.#...#.#.###.#",
        "#.###.#.##.##.#.###.#",
        "#.###.#.#####.#.###.#",
        "#.....#.##..#.#.....#",
        "#######.#.#.#.#######",
        "........##.##........",
        "#.#####.....#.#####..",
        "#.###...##..#..##.###",
        "..##.##.#.##.#....##.",
        "###......#...#.######",
        ".#..###..#.#..##...##",
        "........######..###..",
        "#######..##.#.#..###.",
        "#.....#.##.###..###.#",
        "#.###.#.#...####....#",
        "#.###.#.##..##.#.#...",
        "#.###.#.#.##.........",
        "#.....#...#......##..",
        "#######.#..#......##.",
    ];
    let qr = Qr::encode_with_mask(b"OpenECraft", Some(2)).unwrap();
    let rows: Vec<String> = (0..qr.size).map(|y| (0..qr.size).map(|x| if qr.dark(x, y) { '#' } else { '.' }).collect()).collect();
    assert_eq!(rows, reference);
}

#[test]
fn every_mask_and_version_matches_the_reference() {
    // FNV-1a digests of the modules (row by row) of qrcodegen 1.8 for masks 0 … 7
    // (each read back by zxing-cpp up to version 26); the versions cover several
    // blocks of two lengths and the version information.
    let cases: [(usize, usize, [u64; 8]); 5] = [
        (10, 1, [0xce4e49bb23efeda9, 0x1dc1ae995f1a466b, 0xac7fc107d2549bdb, 0xe8181597a3d47579, 0x947a37a6a838105f, 0x1e85b2eedcd420e5, 0x024af1814466b6e9, 0x6661dd41d26fc29f]),
        (60, 4, [0x478ab455982ed96b, 0xd9569d0def6dc365, 0x2a1449f3f51b87e0, 0xdfd300c70f379c54, 0x2d046b81628137a9, 0x90b99e730a8b7597, 0x3349a3a4f1f0b424, 0x7fb4918d7aacc8fa]),
        (300, 13, [0xca9cf9478aa2d3cf, 0x518e0dfc0ce2b718, 0x41b376219b5879fd, 0x110bb64e757a02ef, 0x1707a47320b87bc1, 0x897dc8e7101cdf7f, 0x1336113991e185e3, 0x21ab72183322662d]),
        (1000, 26, [0x3bc012643bde0d77, 0xd96be8d74d118ee8, 0xd78f6b949cc02d8d, 0xef039d6d6c7d452f, 0x121e6fb427f0fa09, 0xfd38c1596b9d10ac, 0xee96d0e461ee752f, 0x513964d5a75b934d]),
        (2331, 40, [0x365eff78b877c65b, 0xf3470f42d4356224, 0xca8dcc7e9ca40e8d, 0xab008dc1864d8edb, 0xf5cc4ac8809c39f5, 0xce9832d1304cba67, 0x2a3c9e98225310eb, 0x2ea25081b4441489]),
    ];
    for (len, version, digests) in cases {
        let t = if len == 10 { "OpenECraft".to_string() } else { text(len) };
        for (mask, digest) in digests.iter().enumerate() {
            let qr = Qr::encode_with_mask(t.as_bytes(), Some(mask as u8)).unwrap();
            assert_eq!((qr.version, qr.size), (version, version * 4 + 17));
            assert_eq!(fnv(&qr), *digest, "{len} bytes, mask {mask}");
        }
    }
}

#[test]
fn the_mask_with_the_lowest_penalty_is_chosen() {
    // The masks the reference chooses.
    for (t, mask) in [("OpenECraft".to_string(), 0), (text(60), 4), (text(300), 2), (text(1000), 2)] {
        assert_eq!(Qr::encode(&t).unwrap().mask, mask, "{} bytes", t.len());
    }
}
