//! Strict UTF-8 encoding and decoding for XL codepoints

use thiserror::Error;

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("invalid UTF-8")]
pub struct Utf8Error;

pub fn encode(codepoints: &[i64]) -> Result<Vec<u8>, Utf8Error> {
    let mut bytes = Vec::new();
    for &codepoint in codepoints {
        validate_encodable_codepoint(codepoint)?;
        match codepoint {
            0..=0x7f => bytes.push(codepoint as u8),
            0x80..=0x7ff => {
                bytes.push((0xc0 | (codepoint >> 6)) as u8);
                bytes.push(continuation(codepoint));
            }
            0x800..=0xffff => {
                bytes.push((0xe0 | (codepoint >> 12)) as u8);
                bytes.push(continuation(codepoint >> 6));
                bytes.push(continuation(codepoint));
            }
            _ => {
                bytes.push((0xf0 | (codepoint >> 18)) as u8);
                bytes.push(continuation(codepoint >> 12));
                bytes.push(continuation(codepoint >> 6));
                bytes.push(continuation(codepoint));
            }
        }
    }
    Ok(bytes)
}

pub fn decode(bytes: &[u8]) -> Result<Vec<i64>, Utf8Error> {
    let mut codepoints = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let first = bytes[index];
        let (length, minimum, codepoint) = match first {
            0x00..=0x7f => (1, 0, i64::from(first)),
            0xc0..=0xdf => (
                2,
                0x80,
                (i64::from(first & 0x1f) << 6) | i64::from(next_continuation(bytes, index + 1)?),
            ),
            0xe0..=0xef => (
                3,
                0x800,
                (i64::from(first & 0x0f) << 12)
                    | (i64::from(next_continuation(bytes, index + 1)?) << 6)
                    | i64::from(next_continuation(bytes, index + 2)?),
            ),
            0xf0..=0xf7 => (
                4,
                0x10000,
                (i64::from(first & 0x07) << 18)
                    | (i64::from(next_continuation(bytes, index + 1)?) << 12)
                    | (i64::from(next_continuation(bytes, index + 2)?) << 6)
                    | i64::from(next_continuation(bytes, index + 3)?),
            ),
            _ => return Err(Utf8Error),
        };
        if bytes.len() - index < length || codepoint < minimum {
            return Err(Utf8Error);
        }
        validate_decoded_codepoint(codepoint)?;
        codepoints.push(codepoint);
        index += length;
    }
    Ok(codepoints)
}

fn continuation(codepoint: i64) -> u8 {
    (0x80 | (codepoint & 0x3f)) as u8
}

fn next_continuation(bytes: &[u8], index: usize) -> Result<u8, Utf8Error> {
    match bytes.get(index).copied() {
        Some(byte @ 0x80..=0xbf) => Ok(byte & 0x3f),
        _ => Err(Utf8Error),
    }
}

fn validate_encodable_codepoint(codepoint: i64) -> Result<(), Utf8Error> {
    if (0..0x110000).contains(&codepoint) {
        Ok(())
    } else {
        Err(Utf8Error)
    }
}

fn validate_decoded_codepoint(codepoint: i64) -> Result<(), Utf8Error> {
    validate_encodable_codepoint(codepoint)?;
    if (0xd800..0xe000).contains(&codepoint) {
        Err(Utf8Error)
    } else {
        Ok(())
    }
}
