//! Packet field packing, reflected CRCs, and one's-complement checksums

use num_bigint::BigInt;
use num_traits::{One, Signed, ToPrimitive, Zero};

use super::spec_impl::unpack::{self, PrecisionNumber};
use crate::{
    lang::data::value::{Value, ValueArena},
    runner::ExternError,
};

fn bit_width(width: &BigInt) -> Result<usize, ExternError> {
    width
        .to_u64()
        .filter(|width| *width <= ((1_u64 << 62) - 1))
        .and_then(|width| usize::try_from(width).ok())
        .ok_or_else(|| ExternError::Failure(format!("invalid hash bit width: {width}")))
}

// Bigint's remainder is nonnegative and requires a positive divisor
fn remainder(int: &BigInt, int_modulus: &BigInt) -> BigInt {
    let int = int % int_modulus;
    if int.is_negative() {
        int + int_modulus
    } else {
        int
    }
}

pub fn bitwise_neg(int: &BigInt, width: &BigInt) -> Result<BigInt, ExternError> {
    if width <= &BigInt::zero() {
        return Ok(int.clone());
    }
    let width = bit_width(width)?;
    Ok(int ^ ((BigInt::one() << width) - BigInt::one()))
}

fn aligned_width(bits: &PrecisionNumber, alignment: usize) -> Result<usize, ExternError> {
    let width = bit_width(&bits.width)?;
    if !width.is_multiple_of(alignment) {
        return Err(ExternError::Failure(
            "bitslice x[y:z] must have y > z > 0".to_owned(),
        ));
    }
    Ok(width)
}

fn crc(bits: &PrecisionNumber, polynomial: u32, int_init: u32) -> Result<BigInt, ExternError> {
    let width = aligned_width(bits, 8)?;
    let mut int_crc = int_init;
    for idx in (0..width).step_by(8).rev() {
        let byte = ((&bits.int >> idx) & BigInt::from(255_u16))
            .to_u32()
            .ok_or_else(|| ExternError::Failure("invalid CRC byte".to_owned()))?;
        let mut entry = (int_crc ^ byte) & 255;
        for _ in 0..8 {
            entry = if entry & 1 == 0 {
                entry >> 1
            } else {
                (entry >> 1) ^ polynomial
            };
        }
        int_crc = (int_crc >> 8) ^ entry;
    }
    Ok(BigInt::from(int_crc ^ int_init))
}

fn checksum(
    bits: &PrecisionNumber,
    int_init: &BigInt,
    subtract: bool,
) -> Result<BigInt, ExternError> {
    let width = aligned_width(bits, 16)?;
    let int_threshold = BigInt::one() << 16;
    let mask = &int_threshold - BigInt::one();
    let mut int_hash = int_init.clone();
    for idx in (0..width).step_by(16).rev() {
        let int_word = (&bits.int >> idx) & &mask;
        let int_word = if subtract { int_word ^ &mask } else { int_word };
        let int_sum = int_hash + int_word;
        let carry = u8::from(int_sum >= int_threshold);
        int_hash = remainder(&int_sum, &int_threshold) + carry;
    }
    // Complement only the low 16 bits, retaining any higher seed bits
    Ok(int_hash ^ mask)
}

pub fn compute_hash(
    algo: &str,
    int_init: Option<&BigInt>,
    bits: &PrecisionNumber,
) -> Result<BigInt, ExternError> {
    match algo {
        "crc16" => crc(bits, 0xA001, 0),
        "crc32" => crc(bits, 0xEDB88320, u32::MAX),
        "csum16" | "csum16_sub" => checksum(
            bits,
            int_init.unwrap_or(&BigInt::zero()),
            algo == "csum16_sub",
        ),
        "identity" => Ok(bits.int.clone()),
        _ => Err(ExternError::Failure(format!("(TODO: compute_hash) {algo}"))),
    }
}

pub fn package(arena: &ValueArena, values: &[Value]) -> Result<PrecisionNumber, ExternError> {
    let mut bits = PrecisionNumber {
        width: BigInt::zero(),
        int: BigInt::zero(),
    };
    for value in values {
        let bits_field = unpack::p4_precision_number(arena, value)?;
        let width = bit_width(&bits_field.width)?;
        let int_modulus = BigInt::one() << width;
        bits.width += bits_field.width;
        bits.int = (bits.int << width) + remainder(&bits_field.int, &int_modulus);
    }
    let rem = &bits.width % 16_u8;
    if !rem.is_zero() {
        // Source padding widens the capacity without shifting the packed bits
        bits.width += BigInt::from(16_u8) - rem;
    }
    Ok(bits)
}

pub fn compute_checksum(
    algo: &str,
    int_init: Option<&BigInt>,
    arena: &ValueArena,
    values: &[Value],
) -> Result<BigInt, ExternError> {
    let bits = package(arena, values)?;
    compute_hash(algo, int_init, &bits)
}

pub fn adjust(base: &BigInt, rmax: &BigInt, int: &BigInt) -> Result<BigInt, ExternError> {
    if rmax.is_zero() {
        return Ok(base.clone());
    }
    let int_range = rmax - base;
    if int_range <= BigInt::zero() {
        return Err(ExternError::Failure(
            "hash range divisor must be positive".to_owned(),
        ));
    }
    Ok(remainder(int, &int_range) + base)
}
