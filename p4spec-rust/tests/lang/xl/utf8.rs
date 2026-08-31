use super::*;

#[test]
fn test_utf8_round_trips_valid_codepoints() {
    let codepoints = [0x24, 0xa2, 0x20ac, 0x10348];
    let bytes = utf8_impl::encode(&codepoints).unwrap();

    assert_eq!(
        bytes,
        vec![0x24, 0xc2, 0xa2, 0xe2, 0x82, 0xac, 0xf0, 0x90, 0x8d, 0x88]
    );
    assert_eq!(utf8_impl::decode(&bytes).unwrap(), codepoints);
}
#[test]
fn test_utf8_encoder_accepts_surrogates_but_decoder_rejects_them() {
    let surrogate = utf8_impl::encode(&[0xd800]).unwrap();

    assert_eq!(surrogate, vec![0xed, 0xa0, 0x80]);
    assert!(utf8_impl::decode(&surrogate).is_err());
}
#[test]
fn test_utf8_rejects_invalid_codepoints_and_byte_sequences() {
    assert!(utf8_impl::encode(&[-1]).is_err());
    assert!(utf8_impl::encode(&[0x110000]).is_err());

    for bytes in [
        &[0xc0, 0x80][..],
        &[0xed, 0xa0, 0x80][..],
        &[0xf4, 0x90, 0x80, 0x80][..],
        &[0xe2, 0x28, 0xa1][..],
        &[0xf0, 0x90, 0x80][..],
    ] {
        assert!(utf8_impl::decode(bytes).is_err(), "accepted {bytes:02x?}");
    }
}
