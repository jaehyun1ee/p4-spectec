use num_bigint::BigInt;
use num_traits::{One, ToPrimitive, Zero};

use crate::{
    lang::{common::source::Span, il::ast::Typ, xl::num},
    runtime::{
        types::typ as make_type,
        value::{Value, ValueRef, get, make},
    },
};

use super::{BuiltinError, BuiltinResult, extract, return_value};

// Maximum bit width

const MAX_BIT_WIDTH: usize = 2048;

// Conversion between meta-bits and OCaml bool array

fn bits_of_value(span: &Span, value: &Value) -> Result<Vec<bool>, BuiltinError> {
    get::list(value)
        .map_err(|error| BuiltinError::new(span.clone(), error.to_string()))?
        .iter()
        .map(|value| {
            get::bool(value).map_err(|error| BuiltinError::new(span.clone(), error.to_string()))
        })
        .collect()
}

fn value_of_bits(add: &mut dyn FnMut(ValueRef), bits: Vec<bool>) -> BuiltinResult {
    let typ = make_type::var(
        crate::phrase!(node: "bit".to_owned(), span: Span::default()),
        Vec::new(),
    );
    let values_bit = bits
        .into_iter()
        .map(|bit| make::bool(bit, Span::default()))
        .collect();
    return_value(add, make::list(&typ, values_bit, Span::default()))
}

// Conversion between meta-numerics and OCaml numerics

fn bigint_of_value<'a>(span: &Span, value: &'a Value) -> Result<&'a BigInt, BuiltinError> {
    let number =
        get::num(value).map_err(|error| BuiltinError::new(span.clone(), error.to_string()))?;
    Ok(num::to_int(number))
}

fn value_of_bigint(add: &mut dyn FnMut(ValueRef), value: BigInt) -> BuiltinResult {
    return_value(add, make::int(value, Span::default()))
}

fn width_of_bigint(
    span: &Span,
    width: &BigInt,
    too_large: &'static str,
) -> Result<usize, BuiltinError> {
    if width > &BigInt::from(MAX_BIT_WIDTH) {
        return Err(BuiltinError::new(span.clone(), too_large));
    }
    if width <= &BigInt::zero() {
        return Ok(0);
    }
    width
        .to_usize()
        .ok_or_else(|| BuiltinError::new(span.clone(), too_large))
}

fn pow2_value(span: &Span, width: &BigInt) -> Result<BigInt, BuiltinError> {
    if width <= &BigInt::zero() {
        return Ok(BigInt::one());
    }
    let width = width
        .to_usize()
        .ok_or_else(|| BuiltinError::new(span.clone(), "shift amount too large"))?;
    Ok(BigInt::one() << width)
}

// Built-in implementations

// dec $shl(int, int) : int

pub fn shl(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let (value_base, value_offset) = extract::two(span, values)?;
    let base = bigint_of_value(span, value_base)?;
    let offset = bigint_of_value(span, value_offset)?;
    let offset = width_of_bigint(span, offset, "shift amount too large")?;
    value_of_bigint(add, base << offset)
}

// dec $shr(int, int) : int

pub fn shr(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let (value_base, value_offset) = extract::two(span, values)?;
    let base = bigint_of_value(span, value_base)?;
    let offset = bigint_of_value(span, value_offset)?;
    let offset = width_of_bigint(span, offset, "shift amount too large")?;
    value_of_bigint(add, base / (BigInt::one() << offset))
}

// dec $shr_arith(int, int, int) : int

pub fn shr_arith(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let (value_base, value_offset, value_modulus) = extract::three(span, values)?;
    let mut base = bigint_of_value(span, value_base)?.clone();
    let offset = bigint_of_value(span, value_offset)?;
    let offset = width_of_bigint(span, offset, "shift amount too large")?;
    let modulus = bigint_of_value(span, value_modulus)?;
    for _ in 0..offset {
        base = base / 2 + modulus;
    }
    value_of_bigint(add, base)
}

// dec $pow2(int) : int

pub fn pow2(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let value_width = extract::one(span, values)?;
    let width = bigint_of_value(span, value_width)?;
    value_of_bigint(add, pow2_value(span, width)?)
}

// dec $bitstr_to_int(int, bitstr) : int

pub fn bitstr_to_int(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let (value_width, value_bitstr) = extract::two(span, values)?;
    let width = bigint_of_value(span, value_width)?;
    let width = width_of_bigint(span, width, "bitstr width too large")?;
    if width == 0 {
        return value_of_bigint(add, BigInt::zero());
    }
    let modulus = BigInt::one() << width;
    let half = &modulus / 2;
    let bitstr = bigint_of_value(span, value_bitstr)?;
    let normalized = ((bitstr + &half) % &modulus + &modulus) % &modulus - half;
    value_of_bigint(add, normalized)
}

// dec $int_to_bitstr(int, int) : bitstr

pub fn int_to_bitstr(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let (value_width, value_int) = extract::two(span, values)?;
    let width = bigint_of_value(span, value_width)?;
    let width = width_of_bigint(span, width, "bitstr width too large")?;
    if width == 0 {
        return value_of_bigint(add, BigInt::zero());
    }
    let modulus = BigInt::one() << width;
    let rawint = bigint_of_value(span, value_int)?;
    let normalized = (rawint % &modulus + &modulus) % modulus;
    value_of_bigint(add, normalized)
}

// dec $bits_to_int_unsigned(bool* ) : int

fn bits_to_int_unsigned_value(bits: &[bool]) -> BigInt {
    bits.iter().fold(BigInt::zero(), |value, bit| {
        (value << 1) + usize::from(*bit)
    })
}

pub fn bits_to_int_unsigned(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let value_bits = extract::one(span, values)?;
    let bits = bits_of_value(span, value_bits)?;
    value_of_bigint(add, bits_to_int_unsigned_value(&bits))
}

// dec $bits_to_int_signed(bool* ) : int

pub fn bits_to_int_signed(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let value_bits = extract::one(span, values)?;
    let bits = bits_of_value(span, value_bits)?;
    let Some(sign) = bits.first() else {
        return Err(BuiltinError::new(Span::default(), "empty bit array"));
    };
    let mut value = bits_to_int_unsigned_value(&bits);
    if *sign {
        value -= BigInt::one() << bits.len();
    }
    value_of_bigint(add, value)
}

// dec $int_to_bits_unsigned(int) : bool*

fn int_to_bits_unsigned_value(value: &BigInt, width: usize) -> Vec<bool> {
    (0..width)
        .rev()
        .map(|index| ((value >> index) & BigInt::one()) > BigInt::zero())
        .collect()
}

pub fn int_to_bits_unsigned(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let (value_width, value_int) = extract::two(span, values)?;
    let width = bigint_of_value(span, value_width)?;
    let width = width_of_bigint(span, width, "bitstr width too large")?;
    let value = bigint_of_value(span, value_int)?;
    value_of_bits(add, int_to_bits_unsigned_value(value, width))
}

// dec $int_to_bits_signed(int) : bool*

pub fn int_to_bits_signed(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let (value_width, value_int) = extract::two(span, values)?;
    let width = bigint_of_value(span, value_width)?;
    let width = width_of_bigint(span, width, "bitstr width too large")?;
    let value = bigint_of_value(span, value_int)?;
    let mask = (BigInt::one() << width) - 1;
    let value = value & mask;
    value_of_bits(add, int_to_bits_unsigned_value(&value, width))
}

// dec $bneg(int) : int

pub fn bneg(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let value = extract::one(span, values)?;
    let rawint = bigint_of_value(span, value)?;
    value_of_bigint(add, !rawint)
}

// dec $band(int, int) : int

pub fn band(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let (value_l, value_r) = extract::two(span, values)?;
    let rawint_l = bigint_of_value(span, value_l)?;
    let rawint_r = bigint_of_value(span, value_r)?;
    value_of_bigint(add, rawint_l & rawint_r)
}

// dec $bxor(int, int) : int

pub fn bxor(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let (value_l, value_r) = extract::two(span, values)?;
    let rawint_l = bigint_of_value(span, value_l)?;
    let rawint_r = bigint_of_value(span, value_r)?;
    value_of_bigint(add, rawint_l ^ rawint_r)
}

// dec $bor(int, int) : int

pub fn bor(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let (value_l, value_r) = extract::two(span, values)?;
    let rawint_l = bigint_of_value(span, value_l)?;
    let rawint_r = bigint_of_value(span, value_r)?;
    value_of_bigint(add, rawint_l | rawint_r)
}

// dec $bitacc(int, int, int) : int

pub fn bitacc(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let (value_b, value_h, value_l) = extract::three(span, values)?;
    let rawint_b = bigint_of_value(span, value_b)?;
    let rawint_h = bigint_of_value(span, value_h)?;
    let rawint_l = bigint_of_value(span, value_l)?;
    if rawint_l < &BigInt::zero() {
        return Err(BuiltinError::new(
            span.clone(),
            "bitslice x[y:z] must have y > z > 0",
        ));
    }
    let low = rawint_l
        .to_usize()
        .ok_or_else(|| BuiltinError::new(span.clone(), "bitslice index too large"))?;
    let slice_width = rawint_h + 1 - rawint_l;
    let mask = pow2_value(span, &slice_width)? - 1;
    value_of_bigint(add, (rawint_b >> low) & mask)
}

// dec $bitacc_replace(int, int, int, int) : int

pub fn bitacc_replace(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let (value_b, value_h, value_l, value_rhs) = extract::four(span, values)?;
    let rawint_b = bigint_of_value(span, value_b)?;
    let rawint_h = bigint_of_value(span, value_h)?;
    let rawint_l = bigint_of_value(span, value_l)?;
    let rawint_rhs = bigint_of_value(span, value_rhs)?;
    if rawint_l < &BigInt::zero() {
        return Err(BuiltinError::new(
            span.clone(),
            "bitslice x[y:z] must have y > z > 0",
        ));
    }
    let low = rawint_l
        .to_usize()
        .ok_or_else(|| BuiltinError::new(span.clone(), "bitslice index too large"))?;
    let rhs = rawint_rhs << low;
    let mask_hi: BigInt = pow2_value(span, &(rawint_h + 1))? - BigInt::one();
    let mask_lo: BigInt = pow2_value(span, rawint_l)? - BigInt::one();
    let mask: BigInt = !(mask_hi ^ mask_lo);
    value_of_bigint(add, (rawint_b & mask) ^ rhs)
}
