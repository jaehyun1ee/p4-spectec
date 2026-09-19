use std::cmp::Ordering;

use num_bigint::BigInt;
use p4spec_rust::{
    lang::common::prim::num::{
        self as num_impl, BinOp, CmpOp, Natural, Number, NumericError, Typ, UnOp,
    },
    lang::traits::print::Print,
};

fn natural(value: u64) -> Number {
    Number::Nat(value.into())
}

#[path = "prim/num.rs"]
mod num;
