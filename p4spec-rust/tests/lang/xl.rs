use std::cmp::Ordering;

use num_bigint::BigInt;
use p4spec_rust::{
    lang::common::source::{Position, Span},
    lang::traits::print::Print,
    lang::xl::{
        num::{self as num_impl, BinOp, CmpOp, Natural, Number, NumericError, Typ, UnOp},
        utf8 as utf8_impl, var as var_impl,
    },
};

fn natural(value: u64) -> Number {
    Number::Nat(value.into())
}

#[path = "xl/num.rs"]
mod num;
#[path = "xl/utf8.rs"]
mod utf8;
#[path = "xl/var.rs"]
mod var;
