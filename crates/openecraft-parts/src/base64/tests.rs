use super::*;

#[test]
fn round_trip_and_padding() {
    assert_eq!(encode(b"Ma"), "TWE=");
    assert_eq!(encode(b"Man"), "TWFu");
    assert_eq!(encode(b"M"), "TQ==");
    let data: Vec<u8> = (0..=255).collect();
    assert_eq!(decode(&encode(&data)).unwrap(), data);
    assert_eq!(decode("TW\nFu").unwrap(), b"Man");
    assert!(decode("T!==").is_none());
}
