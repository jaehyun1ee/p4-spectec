//! UTF-8 encoding for XL codepoints

use thiserror::Error;

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("invalid UTF-8")]
pub struct Utf8Error;

// Encoder

/// Applies encode
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

fn continuation(codepoint: i64) -> u8 {
    (0x80 | (codepoint & 0x3f)) as u8
}

fn validate_encodable_codepoint(codepoint: i64) -> Result<(), Utf8Error> {
    if (0..0x110000).contains(&codepoint) { Ok(()) } else { Err(Utf8Error) }
}
