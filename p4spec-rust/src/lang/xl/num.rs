use std::{cmp::Ordering, fmt};

use num_bigint::BigInt;
use num_traits::{Signed, Zero};
use thiserror::Error;

// Numbers: natural numbers and integers

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Natural(BigInt);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Number {
    Nat(Natural),
    Int(BigInt),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Typ {
    NatT,
    IntT,
}

pub fn to_typ(number: &Number) -> Typ {
    match number {
        Number::Nat(_) => Typ::NatT,
        Number::Int(_) => Typ::IntT,
    }
}

pub fn to_int(number: &Number) -> &BigInt {
    match number {
        Number::Nat(natural) => natural.as_bigint(),
        Number::Int(integer) => integer,
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

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum NumericError {
    #[error("natural number cannot be negative: {0}")]
    NegativeNatural(BigInt),

    #[error("numeric operands have mismatched kinds: {left:?} and {right:?}")]
    MismatchedKinds { left: Typ, right: Typ },

    #[error("numeric operation {0:?} has a zero divisor")]
    ZeroDivisor(BinOp),

    #[error("unsupported numeric binary operation: {0:?}")]
    UnsupportedBinaryOperation(BinOp),
}

impl Natural {
    pub fn as_bigint(&self) -> &BigInt {
        &self.0
    }
}

impl TryFrom<BigInt> for Natural {
    type Error = NumericError;

    fn try_from(integer: BigInt) -> Result<Self, Self::Error> {
        if integer.is_negative() {
            Err(NumericError::NegativeNatural(integer))
        } else {
            Ok(Self(integer))
        }
    }
}

impl From<u64> for Natural {
    fn from(integer: u64) -> Self {
        Self(integer.into())
    }
}

impl fmt::Display for Natural {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

// Comparison

pub fn compare(number_a: &Number, number_b: &Number) -> Ordering {
    match (number_a, number_b) {
        (Number::Nat(natural_a), Number::Nat(natural_b)) => natural_a.0.cmp(&natural_b.0),
        (Number::Int(integer_a), Number::Int(integer_b)) => integer_a.cmp(integer_b),
        (Number::Nat(_), Number::Int(_)) => Ordering::Less,
        (Number::Int(_), Number::Nat(_)) => Ordering::Greater,
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

pub fn eq(number_a: &Number, number_b: &Number) -> bool {
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

pub fn string_of_num(number: &Number) -> String {
    match number {
        Number::Nat(natural) => natural.to_string(),
        Number::Int(integer) => {
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

pub fn un(operation: UnOp, number: &Number) -> Number {
    match operation {
        UnOp::PlusOp => number.clone(),
        UnOp::MinusOp => Number::Int(-to_int(number)),
    }
}

// Binary

pub fn bin(operation: BinOp, number_l: &Number, number_r: &Number) -> Result<Number, NumericError> {
    match (operation, number_l, number_r) {
        (BinOp::AddOp, Number::Nat(natural_l), Number::Nat(natural_r)) => {
            Ok(Number::Nat(Natural(&natural_l.0 + &natural_r.0)))
        }
        (BinOp::AddOp, Number::Int(integer_l), Number::Int(integer_r)) => {
            Ok(Number::Int(integer_l + integer_r))
        }
        (BinOp::SubOp, Number::Nat(natural_l), Number::Nat(natural_r)) => {
            Ok(Number::Int(&natural_l.0 - &natural_r.0))
        }
        (BinOp::SubOp, Number::Int(integer_l), Number::Int(integer_r)) => {
            Ok(Number::Int(integer_l - integer_r))
        }
        (BinOp::MulOp, Number::Nat(natural_l), Number::Nat(natural_r)) => {
            Ok(Number::Nat(Natural(&natural_l.0 * &natural_r.0)))
        }
        (BinOp::MulOp, Number::Int(integer_l), Number::Int(integer_r)) => {
            Ok(Number::Int(integer_l * integer_r))
        }
        (operation @ (BinOp::DivOp | BinOp::ModOp), Number::Nat(_), Number::Nat(natural_r))
            if natural_r.0.is_zero() =>
        {
            Err(NumericError::ZeroDivisor(operation))
        }
        (operation @ (BinOp::DivOp | BinOp::ModOp), Number::Int(_), Number::Int(integer_r))
            if integer_r.is_zero() =>
        {
            Err(NumericError::ZeroDivisor(operation))
        }
        (BinOp::DivOp, Number::Nat(natural_l), Number::Nat(natural_r)) => {
            Ok(Number::Nat(Natural(&natural_l.0 / &natural_r.0)))
        }
        (BinOp::DivOp, Number::Int(integer_l), Number::Int(integer_r)) => {
            Ok(Number::Int(integer_l / integer_r))
        }
        (BinOp::ModOp, Number::Nat(natural_l), Number::Nat(natural_r)) => {
            Ok(Number::Nat(Natural(&natural_l.0 % &natural_r.0)))
        }
        (BinOp::ModOp, Number::Int(integer_l), Number::Int(integer_r)) => {
            Ok(Number::Int(integer_l % integer_r))
        }
        (BinOp::PowOp, Number::Nat(_), Number::Nat(_))
        | (BinOp::PowOp, Number::Int(_), Number::Int(_)) => {
            Err(NumericError::UnsupportedBinaryOperation(operation))
        }
        (_, number_l, number_r) => Err(NumericError::MismatchedKinds {
            left: to_typ(number_l),
            right: to_typ(number_r),
        }),
    }
}

// Comparison

pub fn cmp(operation: CmpOp, number_l: &Number, number_r: &Number) -> Result<bool, NumericError> {
    match (operation, number_l, number_r) {
        (CmpOp::LtOp, Number::Nat(natural_l), Number::Nat(natural_r)) => {
            Ok(natural_l.0 < natural_r.0)
        }
        (CmpOp::LtOp, Number::Int(integer_l), Number::Int(integer_r)) => Ok(integer_l < integer_r),
        (CmpOp::GtOp, Number::Nat(natural_l), Number::Nat(natural_r)) => {
            Ok(natural_l.0 > natural_r.0)
        }
        (CmpOp::GtOp, Number::Int(integer_l), Number::Int(integer_r)) => Ok(integer_l > integer_r),
        (CmpOp::LeOp, Number::Nat(natural_l), Number::Nat(natural_r)) => {
            Ok(natural_l.0 <= natural_r.0)
        }
        (CmpOp::LeOp, Number::Int(integer_l), Number::Int(integer_r)) => Ok(integer_l <= integer_r),
        (CmpOp::GeOp, Number::Nat(natural_l), Number::Nat(natural_r)) => {
            Ok(natural_l.0 >= natural_r.0)
        }
        (CmpOp::GeOp, Number::Int(integer_l), Number::Int(integer_r)) => Ok(integer_l >= integer_r),
        (_, number_l, number_r) => Err(NumericError::MismatchedKinds {
            left: to_typ(number_l),
            right: to_typ(number_r),
        }),
    }
}
