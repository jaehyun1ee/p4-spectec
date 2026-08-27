use std::str::FromStr;

use num_bigint::BigInt;
use serde_json::{Value, json};

use crate::lang::xl::num;

use super::super::{DecodeError, variant};

pub(super) fn decode_num(value: &Value) -> Result<num::Number, DecodeError> {
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
        ("Nat", [integer]) => Ok(num::Number::Nat(
            num::Natural::try_from(decode_bigint(integer)?)
                .map_err(|_| DecodeError::Expected("non-negative natural number"))?,
        )),
        ("Int", [integer]) => Ok(num::Number::Int(decode_bigint(integer)?)),
        ("Nat" | "Int", _) => Err(DecodeError::Expected("valid number arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

pub(super) fn encode_num(num: &num::Number) -> Value {
    match num {
        num::Number::Nat(integer) => json!(["Nat", integer.to_string()]),
        num::Number::Int(integer) => json!(["Int", integer.to_string()]),
    }
}

pub(super) fn decode_num_typ(value: &Value) -> Result<num::Typ, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("NatT", []) => Ok(num::Typ::Nat),
        ("IntT", []) => Ok(num::Typ::Int),
        ("NatT" | "IntT", _) => Err(DecodeError::Expected("valid number type arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

pub(super) fn encode_num_typ(typ: num::Typ) -> Value {
    match typ {
        num::Typ::Nat => json!(["NatT"]),
        num::Typ::Int => json!(["IntT"]),
    }
}
