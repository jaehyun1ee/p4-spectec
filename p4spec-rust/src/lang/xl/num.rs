use std::cmp::Ordering;

use num_bigint::BigInt;
use num_traits::{Signed, Zero};

// Numbers: natural numbers and integers

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum T {
    Nat(BigInt),
    Int(BigInt),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Typ {
    NatT,
    IntT,
}

pub fn to_typ(number: &T) -> Typ {
    match number {
        T::Nat(_) => Typ::NatT,
        T::Int(_) => Typ::IntT,
    }
}

pub fn to_int(number: &T) -> &BigInt {
    match number {
        T::Nat(integer) | T::Int(integer) => integer,
    }
}

// Operations

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnOp {
    PlusOp,
    MinusOp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinOp {
    AddOp,
    SubOp,
    MulOp,
    DivOp,
    ModOp,
    PowOp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CmpOp {
    LtOp,
    GtOp,
    LeOp,
    GeOp,
}

// Comparison

pub fn compare(number_a: &T, number_b: &T) -> Ordering {
    match (number_a, number_b) {
        (T::Nat(natural_a), T::Nat(natural_b)) => natural_a.cmp(natural_b),
        (T::Int(integer_a), T::Int(integer_b)) => integer_a.cmp(integer_b),
        (T::Nat(_), T::Int(_)) => Ordering::Less,
        (T::Int(_), T::Nat(_)) => Ordering::Greater,
    }
}

pub fn compare_typ(type_a: Typ, type_b: Typ) -> Ordering {
    match (type_a, type_b) {
        (Typ::NatT, Typ::NatT) | (Typ::IntT, Typ::IntT) => Ordering::Equal,
        (Typ::NatT, Typ::IntT) => Ordering::Less,
        (Typ::IntT, Typ::NatT) => Ordering::Greater,
    }
}

// Equality

pub fn eq(number_a: &T, number_b: &T) -> bool {
    compare(number_a, number_b) == Ordering::Equal
}

// Subtyping

pub fn equiv(type_a: Typ, type_b: Typ) -> bool {
    type_a == type_b
}

pub fn sub(type_a: Typ, type_b: Typ) -> bool {
    matches!((type_a, type_b), (Typ::NatT, Typ::IntT)) || equiv(type_a, type_b)
}

// Stringifiers

pub fn string_of_num(number: &T) -> String {
    match number {
        T::Nat(natural) => natural.to_string(),
        T::Int(integer) => {
            let sign = if integer.is_negative() { "-" } else { "+" };
            format!("{sign}{}", integer.abs())
        }
    }
}

pub fn string_of_typ(number_type: Typ) -> &'static str {
    match number_type {
        Typ::NatT => "nat",
        Typ::IntT => "int",
    }
}

pub fn string_of_unop(operation: UnOp) -> &'static str {
    match operation {
        UnOp::PlusOp => "+",
        UnOp::MinusOp => "-",
    }
}

pub fn string_of_binop(operation: BinOp) -> &'static str {
    match operation {
        BinOp::AddOp => "+",
        BinOp::SubOp => "-",
        BinOp::MulOp => "*",
        BinOp::DivOp => "/",
        BinOp::ModOp => "\\",
        BinOp::PowOp => "^",
    }
}

pub fn string_of_cmpop(operation: CmpOp) -> &'static str {
    match operation {
        CmpOp::LtOp => "<",
        CmpOp::GtOp => ">",
        CmpOp::LeOp => "<=",
        CmpOp::GeOp => ">=",
    }
}

// Unary

pub fn un(operation: UnOp, number: &T) -> T {
    match operation {
        UnOp::PlusOp => number.clone(),
        UnOp::MinusOp => T::Int(-to_int(number)),
    }
}

// Binary

pub fn bin(operation: BinOp, number_l: &T, number_r: &T) -> T {
    match (operation, number_l, number_r) {
        (BinOp::AddOp, T::Nat(natural_l), T::Nat(natural_r)) => T::Nat(natural_l + natural_r),
        (BinOp::AddOp, T::Int(integer_l), T::Int(integer_r)) => T::Int(integer_l + integer_r),
        (BinOp::SubOp, T::Nat(natural_l), T::Nat(natural_r)) => T::Int(natural_l - natural_r),
        (BinOp::SubOp, T::Int(integer_l), T::Int(integer_r)) => T::Int(integer_l - integer_r),
        (BinOp::MulOp, T::Nat(natural_l), T::Nat(natural_r)) => T::Nat(natural_l * natural_r),
        (BinOp::MulOp, T::Int(integer_l), T::Int(integer_r)) => T::Int(integer_l * integer_r),
        (BinOp::DivOp, T::Nat(natural_l), T::Nat(natural_r)) if !natural_r.is_zero() => {
            T::Nat(natural_l / natural_r)
        }
        (BinOp::DivOp, T::Int(integer_l), T::Int(integer_r)) if !integer_r.is_zero() => {
            T::Int(integer_l / integer_r)
        }
        (BinOp::ModOp, T::Nat(natural_l), T::Nat(natural_r)) if !natural_r.is_zero() => {
            T::Nat(natural_l % natural_r)
        }
        (BinOp::ModOp, T::Int(integer_l), T::Int(integer_r)) if !integer_r.is_zero() => {
            T::Int(integer_l % integer_r)
        }
        _ => panic!("invalid numeric binary operation"),
    }
}

// Comparison

pub fn cmp(operation: CmpOp, number_l: &T, number_r: &T) -> bool {
    match (operation, number_l, number_r) {
        (CmpOp::LtOp, T::Nat(natural_l), T::Nat(natural_r)) => natural_l < natural_r,
        (CmpOp::LtOp, T::Int(integer_l), T::Int(integer_r)) => integer_l < integer_r,
        (CmpOp::GtOp, T::Nat(natural_l), T::Nat(natural_r)) => natural_l > natural_r,
        (CmpOp::GtOp, T::Int(integer_l), T::Int(integer_r)) => integer_l > integer_r,
        (CmpOp::LeOp, T::Nat(natural_l), T::Nat(natural_r)) => natural_l <= natural_r,
        (CmpOp::LeOp, T::Int(integer_l), T::Int(integer_r)) => integer_l <= integer_r,
        (CmpOp::GeOp, T::Nat(natural_l), T::Nat(natural_r)) => natural_l >= natural_r,
        (CmpOp::GeOp, T::Int(integer_l), T::Int(integer_r)) => integer_l >= integer_r,
        _ => panic!("invalid numeric comparison"),
    }
}
