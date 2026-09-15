use std::str::FromStr;

use num_bigint::BigInt;
use serde_json::json;

use crate::util::json::json;

use crate::lang::xl::num;

use super::{DecodeError, variant};

pub(crate) fn decode_num(json: &json) -> Result<num::Number, DecodeError> {
    let (tag, fields) = variant(json)?;
    let decode_bigint = |json: &json| {
        let decimal = if let Some(decimal) = json.as_str() {
            decimal.to_owned()
        } else if let Some(int) = json.as_i64() {
            int.to_string()
        } else {
            return Err(DecodeError::Expected("decimal bigint string or integer"));
        };
        BigInt::from_str(&decimal).map_err(|_| DecodeError::Expected("valid decimal bigint"))
    };

    match (tag, fields) {
        ("Nat", [int]) => Ok(num::Number::Nat(
            num::Natural::try_from(decode_bigint(int)?)
                .map_err(|_| DecodeError::Expected("non-negative natural number"))?,
        )),
        ("Int", [int]) => Ok(num::Number::Int(decode_bigint(int)?)),
        ("Nat" | "Int", _) => Err(DecodeError::Expected("valid number arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

pub(crate) fn encode_num(num: &num::Number) -> json {
    match num {
        num::Number::Nat(int) => json!(["Nat", int.to_string()]),
        num::Number::Int(int) => json!(["Int", int.to_string()]),
    }
}

pub(crate) fn decode_num_typ(json: &json) -> Result<num::Typ, DecodeError> {
    let (tag, fields) = variant(json)?;
    match (tag, fields) {
        ("NatT", []) => Ok(num::Typ::Nat),
        ("IntT", []) => Ok(num::Typ::Int),
        ("NatT" | "IntT", _) => Err(DecodeError::Expected("valid number type arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

pub(crate) fn encode_num_typ(typ: num::Typ) -> json {
    match typ {
        num::Typ::Nat => json!(["NatT"]),
        num::Typ::Int => json!(["IntT"]),
    }
}
