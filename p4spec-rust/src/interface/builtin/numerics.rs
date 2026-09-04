//! Bit and integer builtins in specification order.
//!
//! Calls decode runtime numerics, apply the bounded arithmetic operation, and
//! return one encoded result. For example, unsigned bits `[true, false]`
//! decode to the integer `2`.

use std::rc::Rc;

use num_bigint::BigInt;
use num_traits::{One, ToPrimitive, Zero};

use crate::lang::{
    common::source::Span,
    data::{
        typ,
        value::{Value, get, make},
    },
    il::ast::Typ,
    xl::num,
};

use super::{BuiltinError, extract};

// == Maximum bit width

const MAX_BIT_WIDTH: usize = 2048;

// == Conversion between meta-bits and bit vectors

fn bits_of_value(value: &Value) -> Result<Vec<bool>, BuiltinError> {
    let values = get::list(value).map_err(|error| BuiltinError::new(error.to_string()))?;
    let mut bits = Vec::with_capacity(values.len());
    for value in values {
        let bit = get::bool(value).map_err(|error| BuiltinError::new(error.to_string()))?;
        bits.push(bit);
    }
    Ok(bits)
}

fn value_of_bits(bits: Vec<bool>) -> Result<Rc<Value>, BuiltinError> {
    let bit_id = crate::phrase!(node: "bit".to_owned(), span: Span::default());
    let typ = typ::make::var(bit_id, Vec::new());
    let mut bit_values = Vec::with_capacity(bits.len());
    for bit in bits {
        let bit_value = make::bool(bit, Span::default());
        bit_values.push(bit_value);
    }
    let value = make::list(&typ, bit_values, Span::default());
    Ok(value)
}

// == Conversion between meta-numerics and runtime numerics

fn bigint_of_value(value: &Value) -> Result<&BigInt, BuiltinError> {
    let number = get::num(value).map_err(|error| BuiltinError::new(error.to_string()))?;
    Ok(num::to_int(number))
}

fn value_of_bigint(value: BigInt) -> Result<Rc<Value>, BuiltinError> {
    let value = make::int(value, Span::default());
    Ok(value)
}

fn width_of_bigint(width: &BigInt, too_large: &'static str) -> Result<usize, BuiltinError> {
    if width > &BigInt::from(MAX_BIT_WIDTH) {
        return Err(BuiltinError::new(too_large));
    }
    if width <= &BigInt::zero() {
        return Ok(0);
    }
    width.to_usize().ok_or_else(|| BuiltinError::new(too_large))
}

fn array_width_of_bigint(width: &BigInt) -> Result<usize, BuiltinError> {
    if width < &BigInt::zero() {
        return Err(BuiltinError::new("negative bit array width"));
    }
    width_of_bigint(width, "bitstr width too large")
}

fn pow2_value(width: &BigInt) -> Result<BigInt, BuiltinError> {
    if width <= &BigInt::zero() {
        return Ok(BigInt::one());
    }
    let width = width
        .to_usize()
        .ok_or_else(|| BuiltinError::new("shift amount too large"))?;
    let value = BigInt::one() << width;
    Ok(value)
}

// == Built-in implementations

// dec $shl(int, int) : int

pub fn shl(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    extract::zero(targs)?;
    let (value_base, value_offset) = extract::two(values)?;
    let base = bigint_of_value(value_base)?;
    let offset = bigint_of_value(value_offset)?;
    let offset = width_of_bigint(offset, "shift amount too large")?;
    let value = base << offset;
    value_of_bigint(value)
}

// dec $shr(int, int) : int

pub fn shr(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    extract::zero(targs)?;
    let (value_base, value_offset) = extract::two(values)?;
    let base = bigint_of_value(value_base)?;
    let offset = bigint_of_value(value_offset)?;
    let offset = width_of_bigint(offset, "shift amount too large")?;
    let divisor = BigInt::one() << offset;
    let value = base / divisor;
    value_of_bigint(value)
}

// dec $shr_arith(int, int, int) : int

pub fn shr_arith(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    extract::zero(targs)?;
    let (value_base, value_offset, value_modulus) = extract::three(values)?;
    let base = bigint_of_value(value_base)?;
    let mut base = base.clone();
    let offset = bigint_of_value(value_offset)?;
    let offset = width_of_bigint(offset, "shift amount too large")?;
    let modulus = bigint_of_value(value_modulus)?;
    for _ in 0..offset {
        base = base / 2 + modulus;
    }
    value_of_bigint(base)
}

// dec $pow2(int) : int

pub fn pow2(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    extract::zero(targs)?;
    let value_width = extract::one(values)?;
    let width = bigint_of_value(value_width)?;
    let value = pow2_value(width)?;
    value_of_bigint(value)
}

// dec $bitstr_to_int(int, bitstr) : int

pub fn bitstr_to_int(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    extract::zero(targs)?;
    let (value_width, value_bitstr) = extract::two(values)?;
    let width = bigint_of_value(value_width)?;
    let width = width_of_bigint(width, "bitstr width too large")?;
    if width == 0 {
        return value_of_bigint(BigInt::zero());
    }
    let modulus = BigInt::one() << width;
    let half = &modulus / 2;
    let bitstr = bigint_of_value(value_bitstr)?;
    let normalized = ((bitstr + &half) % &modulus + &modulus) % &modulus - half;
    value_of_bigint(normalized)
}

// dec $int_to_bitstr(int, int) : bitstr

pub fn int_to_bitstr(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    extract::zero(targs)?;
    let (value_width, value_int) = extract::two(values)?;
    let width = bigint_of_value(value_width)?;
    let width = width_of_bigint(width, "bitstr width too large")?;
    if width == 0 {
        return value_of_bigint(BigInt::zero());
    }
    let modulus = BigInt::one() << width;
    let rawint = bigint_of_value(value_int)?;
    let normalized = (rawint % &modulus + &modulus) % modulus;
    value_of_bigint(normalized)
}

// dec $bits_to_int_unsigned(bool*) : int

fn bits_to_int_unsigned_value(bits: &[bool]) -> BigInt {
    bits.iter().fold(BigInt::zero(), |value, bit| {
        (value << 1) + usize::from(*bit)
    })
}

pub fn bits_to_int_unsigned(
    targs: &[Typ],
    values: &[Rc<Value>],
) -> Result<Rc<Value>, BuiltinError> {
    extract::zero(targs)?;
    let value_bits = extract::one(values)?;
    let bits = bits_of_value(value_bits)?;
    let value = bits_to_int_unsigned_value(&bits);
    value_of_bigint(value)
}

// dec $bits_to_int_signed(bool*) : int

pub fn bits_to_int_signed(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    extract::zero(targs)?;
    let value_bits = extract::one(values)?;
    let bits = bits_of_value(value_bits)?;
    let Some(sign) = bits.first() else {
        return Err(BuiltinError::new("empty bit array"));
    };
    let mut value = bits_to_int_unsigned_value(&bits);
    if *sign {
        value -= BigInt::one() << bits.len();
    }
    value_of_bigint(value)
}

// dec $int_to_bits_unsigned(int) : bool*

fn int_to_bits_unsigned_value(value: &BigInt, width: usize) -> Vec<bool> {
    (0..width)
        .rev()
        .map(|index| ((value >> index) & BigInt::one()) > BigInt::zero())
        .collect()
}

pub fn int_to_bits_unsigned(
    targs: &[Typ],
    values: &[Rc<Value>],
) -> Result<Rc<Value>, BuiltinError> {
    extract::zero(targs)?;
    let (value_width, value_int) = extract::two(values)?;
    let width = bigint_of_value(value_width)?;
    let width = array_width_of_bigint(width)?;
    let value = bigint_of_value(value_int)?;
    let bits = int_to_bits_unsigned_value(value, width);
    value_of_bits(bits)
}

// dec $int_to_bits_signed(int) : bool*

pub fn int_to_bits_signed(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    extract::zero(targs)?;
    let (value_width, value_int) = extract::two(values)?;
    let width = bigint_of_value(value_width)?;
    let width = array_width_of_bigint(width)?;
    let value = bigint_of_value(value_int)?;
    let mask = (BigInt::one() << width) - 1;
    let value = value & mask;
    let bits = int_to_bits_unsigned_value(&value, width);
    value_of_bits(bits)
}

// dec $bneg(int) : int

pub fn bneg(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    extract::zero(targs)?;
    let value = extract::one(values)?;
    let rawint = bigint_of_value(value)?;
    value_of_bigint(!rawint)
}

// dec $band(int, int) : int

pub fn band(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    extract::zero(targs)?;
    let (value_l, value_r) = extract::two(values)?;
    let rawint_l = bigint_of_value(value_l)?;
    let rawint_r = bigint_of_value(value_r)?;
    value_of_bigint(rawint_l & rawint_r)
}

// dec $bxor(int, int) : int

pub fn bxor(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    extract::zero(targs)?;
    let (value_l, value_r) = extract::two(values)?;
    let rawint_l = bigint_of_value(value_l)?;
    let rawint_r = bigint_of_value(value_r)?;
    value_of_bigint(rawint_l ^ rawint_r)
}

// dec $bor(int, int) : int

pub fn bor(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    extract::zero(targs)?;
    let (value_l, value_r) = extract::two(values)?;
    let rawint_l = bigint_of_value(value_l)?;
    let rawint_r = bigint_of_value(value_r)?;
    value_of_bigint(rawint_l | rawint_r)
}

// dec $bitacc(int, int, int) : int

pub fn bitacc(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    extract::zero(targs)?;
    let (value_b, value_h, value_l) = extract::three(values)?;
    let rawint_b = bigint_of_value(value_b)?;
    let rawint_h = bigint_of_value(value_h)?;
    let rawint_l = bigint_of_value(value_l)?;
    if rawint_l < &BigInt::zero() {
        return Err(BuiltinError::new("bitslice x[y:z] must have y > z > 0"));
    }
    let low = rawint_l
        .to_usize()
        .ok_or_else(|| BuiltinError::new("bitslice index too large"))?;
    let slice_width = rawint_h + 1 - rawint_l;
    let mask = pow2_value(&slice_width)? - 1;
    let shifted = rawint_b >> low;
    let value = shifted & mask;
    value_of_bigint(value)
}

// dec $bitacc_replace(int, int, int, int) : int

pub fn bitacc_replace(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    extract::zero(targs)?;
    let (value_b, value_h, value_l, value_rhs) = extract::four(values)?;
    let rawint_b = bigint_of_value(value_b)?;
    let rawint_h = bigint_of_value(value_h)?;
    let rawint_l = bigint_of_value(value_l)?;
    let rawint_rhs = bigint_of_value(value_rhs)?;
    if rawint_l < &BigInt::zero() {
        return Err(BuiltinError::new("bitslice x[y:z] must have y > z > 0"));
    }
    let low = rawint_l
        .to_usize()
        .ok_or_else(|| BuiltinError::new("bitslice index too large"))?;
    let rhs = rawint_rhs << low;
    let mask_hi_width = rawint_h + 1;
    let mask_hi: BigInt = pow2_value(&mask_hi_width)? - BigInt::one();
    let mask_lo: BigInt = pow2_value(rawint_l)? - BigInt::one();
    let mask: BigInt = !(mask_hi ^ mask_lo);
    let value = (rawint_b & mask) ^ rhs;
    value_of_bigint(value)
}
