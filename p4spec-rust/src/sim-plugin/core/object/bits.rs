use crate::runner::ExternError;
use num_bigint::BigInt;
use num_traits::{One, Zero};

// Bit manipulation

pub fn string_to_bits(text: &str) -> Result<Vec<bool>, ExternError> {
    let mut bits = Vec::with_capacity(text.len().saturating_mul(4));
    for char in text.chars() {
        let int = char.to_digit(16).ok_or_else(|| {
            ExternError::Failure(format!("invalid hexadecimal packet digit: {char}"))
        })?;
        for idx in (0..4).rev() {
            bits.push(int & (1 << idx) != 0);
        }
    }
    Ok(bits)
}

pub fn bits_to_string(bits: &[bool]) -> String {
    bits.chunks(4)
        .map(|bits| {
            let int = bits
                .iter()
                .fold(0_u8, |int, bit| (int << 1) | u8::from(*bit))
                << (4 - bits.len());
            char::from(b"0123456789ABCDEF"[usize::from(int)])
        })
        .collect()
}

pub fn bits_to_int_unsigned(bits: &[bool]) -> BigInt {
    bits.iter()
        .fold(BigInt::zero(), |int, bit| (int << 1) + u8::from(*bit))
}

pub fn bits_to_int_signed(bits: &[bool]) -> Result<BigInt, ExternError> {
    let sign = bits
        .first()
        .ok_or_else(|| ExternError::Failure("empty signed bit string".to_owned()))?;
    let int = bits_to_int_unsigned(bits);
    Ok(if *sign {
        int - (BigInt::one() << bits.len())
    } else {
        int
    })
}

pub fn int_to_bits_unsigned(int: &BigInt, size: usize) -> Vec<bool> {
    (0..size)
        .rev()
        .map(|idx| (int & (BigInt::one() << idx)) > BigInt::zero())
        .collect()
}

pub fn int_to_bits_signed(int: &BigInt, size: usize) -> Vec<bool> {
    let int = int & ((BigInt::one() << size) - BigInt::one());
    int_to_bits_unsigned(&int, size)
}
