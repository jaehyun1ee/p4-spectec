use std::str::FromStr;

use num_bigint::BigInt;
use serde_json::{Value, json};

use crate::lang::xl::num;

use super::super::{DecodeError, variant};

pub(super) fn decode_num(value: &Value) -> Result<num::T, DecodeError> {
    let (tag, fields) = variant(value)?;
    let decode_bigint = |value: &Value| {
        let decimal = if let Some(decimal) = value.as_str() {
            decimal.to_owned()
        } else if let Some(integer) = value.as_i64() {
            integer.to_string()
        } else {
            return Err(DecodeError::Expected("decimal bigint string or integer"));
        };
        BigInt::from_str(&decimal).map_err(|_| DecodeError::Expected("valid decimal bigint"))
    };

    match (tag, fields) {
        ("Nat", [integer]) => Ok(num::T::Nat(decode_bigint(integer)?)),
        ("Int", [integer]) => Ok(num::T::Int(decode_bigint(integer)?)),
        ("Nat" | "Int", _) => Err(DecodeError::Expected("valid number arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

pub(super) fn encode_num(num: &num::T) -> Value {
    match num {
        num::T::Nat(integer) => json!(["Nat", integer.to_string()]),
        num::T::Int(integer) => json!(["Int", integer.to_string()]),
    }
}

pub(super) fn decode_num_typ(value: &Value) -> Result<num::Typ, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("NatT", []) => Ok(num::Typ::NatT),
        ("IntT", []) => Ok(num::Typ::IntT),
        ("NatT" | "IntT", _) => Err(DecodeError::Expected("valid number type arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

pub(super) fn encode_num_typ(typ: num::Typ) -> Value {
    match typ {
        num::Typ::NatT => json!(["NatT"]),
        num::Typ::IntT => json!(["IntT"]),
    }
}
