use std::cmp::Ordering;

use num_bigint::BigInt;
use p4spec_rust::{
    lang::common::source::{Position, Span},
    lang::traits::print::Print,
    lang::xl::{
        num::{self as num_impl, BinOp, CmpOp, Natural, Number, NumericError, Typ, UnOp},
        var as var_impl,
    },
};

fn natural(value: u64) -> Number {
    Number::Nat(value.into())
}

#[path = "xl/num.rs"]
mod num;
#[path = "xl/var.rs"]
mod var;
