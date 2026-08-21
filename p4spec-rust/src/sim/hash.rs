use num_bigint::BigInt;
use num_traits::{One, ToPrimitive, Zero};

use crate::runtime::value::ValueRef;

use super::{SimError, core::unpack_p4_precision_number};

pub fn compute_checksum(
    algorithm: &str,
    values: &[ValueRef],
    initial: &BigInt,
) -> Result<BigInt, SimError> {
    let (width, value) = package(values)?;
    compute_hash(algorithm, &width, &value, initial)
}

pub fn compute_hash(
    algorithm: &str,
    width: &BigInt,
    value: &BigInt,
    initial: &BigInt,
) -> Result<BigInt, SimError> {
    let width = width
        .to_usize()
        .ok_or_else(|| SimError::message("hash width does not fit usize"))?;
    match algorithm {
        "identity" => Ok(value.clone()),
        "crc16" => crc(width, value, 0, 0xA001, 16),
        "crc32" => crc(width, value, 0xFFFF_FFFF, 0xEDB8_8320, 32)
            .map(|value| value ^ BigInt::from(0xFFFF_FFFF_u32)),
        "csum16" => checksum16(width, value, initial, false),
        "csum16_sub" => checksum16(width, value, initial, true),
        _ => Err(SimError::message(format!(
            "unsupported hash algorithm `{algorithm}`"
        ))),
    }
}

pub fn bitwise_neg(value: &BigInt, width: usize) -> BigInt {
    if width == 0 {
        return value.clone();
    }
    let mask = (BigInt::one() << width) - BigInt::one();
    &mask ^ (value & &mask)
}

fn package(values: &[ValueRef]) -> Result<(BigInt, BigInt), SimError> {
    let (mut width, value) = values.iter().try_fold(
        (0_usize, BigInt::zero()),
        |(packed_width, packed_value), value| {
            let (width, value) = unpack_p4_precision_number(value)?;
            let width = width
                .to_usize()
                .ok_or_else(|| SimError::message("P4 number width does not fit usize"))?;
            let modulus = BigInt::one() << width;
            let value = ((value % &modulus) + &modulus) % &modulus;
            Ok::<_, SimError>((packed_width + width, (packed_value << width) + value))
        },
    )?;
    if width % 16 != 0 {
        width += 16 - width % 16;
    }
    Ok((BigInt::from(width), value))
}

fn crc(
    width: usize,
    value: &BigInt,
    initial: u32,
    polynomial: u32,
    output_width: usize,
) -> Result<BigInt, SimError> {
    if width % 8 != 0 {
        return Err(SimError::message("CRC input width must be byte aligned"));
    }
    let mut hash = initial;
    for offset in (0..width).step_by(8).rev() {
        let byte = ((value >> offset) & BigInt::from(0xFF_u8))
            .to_u8()
            .ok_or_else(|| SimError::message("CRC byte does not fit u8"))?;
        hash ^= u32::from(byte);
        for _ in 0..8 {
            hash = if hash & 1 == 1 {
                (hash >> 1) ^ polynomial
            } else {
                hash >> 1
            };
        }
    }
    let mask = if output_width == 32 {
        u32::MAX
    } else {
        (1_u32 << output_width) - 1
    };
    Ok(BigInt::from(hash & mask))
}

fn checksum16(
    width: usize,
    value: &BigInt,
    initial: &BigInt,
    subtract: bool,
) -> Result<BigInt, SimError> {
    if width % 16 != 0 {
        return Err(SimError::message(
            "Internet checksum input width must be 16-bit aligned",
        ));
    }
    let mut hash = initial.clone();
    let threshold = BigInt::one() << 16;
    for offset in (0..width).step_by(16).rev() {
        let mut word = (value >> offset) & BigInt::from(0xFFFF_u32);
        if subtract {
            word = bitwise_neg(&word, 16);
        }
        let sum = hash + word;
        hash = if sum >= threshold {
            (sum % &threshold) + BigInt::one()
        } else {
            sum % &threshold
        };
    }
    Ok(bitwise_neg(&hash, 16))
}
