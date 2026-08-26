//! Numeric values and operations

use std::{cmp::Ordering, fmt};

use num_bigint::BigInt;
use num_traits::{Signed, Zero};
use thiserror::Error;

// Numbers: natural numbers and integers

/// A non-negative arbitrary-precision integer
///
/// Construct with `TryFrom<BigInt>`;
/// negative inputs return `NumericError::NegativeNatural`
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Natural(BigInt);

/// A natural number or a signed integer
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Number {
    Nat(Natural),
    Int(BigInt),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Typ {
    Nat,
    Int,
}

/// Converts to typ
pub fn to_typ(number: &Number) -> Typ {
    match number {
        Number::Nat(_) => Typ::Nat,
        Number::Int(_) => Typ::Int,
    }
}

/// Converts to int
pub fn to_int(number: &Number) -> &BigInt {
    match number {
        Number::Nat(natural) => natural.as_bigint(),
        Number::Int(integer) => integer,
    }
}

// Operations

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnOp {
    Plus,
    Minus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CmpOp {
    Lt,
    Gt,
    Le,
    Ge,
}

/// Errors from checked numeric construction and operations
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum NumericError {
    #[error("natural number cannot be negative: {0}")]
    NegativeNatural(BigInt),

    #[error("numeric operands have mismatched kinds: {typ_l:?} and {typ_r:?}")]
    MismatchedKinds { typ_l: Typ, typ_r: Typ },

    #[error("numeric operation {0:?} has a zero divisor")]
    ZeroDivisor(BinOp),

    #[error("unsupported numeric binary operation: {0:?}")]
    UnsupportedBinaryOperation(BinOp),
}

impl Natural {
    /// Borrows the validated integer payload
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

/// Compares number kind before numeric value;
/// every natural number sorts before every signed integer
pub fn compare(number_a: &Number, number_b: &Number) -> Ordering {
    match (number_a, number_b) {
        (Number::Nat(natural_a), Number::Nat(natural_b)) => natural_a.0.cmp(&natural_b.0),
        (Number::Int(integer_a), Number::Int(integer_b)) => integer_a.cmp(integer_b),
        (Number::Nat(_), Number::Int(_)) => Ordering::Less,
        (Number::Int(_), Number::Nat(_)) => Ordering::Greater,
    }
}

/// Compares typ
pub fn compare_typ(type_a: Typ, type_b: Typ) -> Ordering {
    match (type_a, type_b) {
        (Typ::Nat, Typ::Nat) | (Typ::Int, Typ::Int) => Ordering::Equal,
        (Typ::Nat, Typ::Int) => Ordering::Less,
        (Typ::Int, Typ::Nat) => Ordering::Greater,
    }
}

// Equality

/// Compares numeric value with number-kind sensitivity
pub fn eq(number_a: &Number, number_b: &Number) -> bool {
    compare(number_a, number_b) == Ordering::Equal
}

// Subtyping

/// Checks equality of uiv
pub fn equiv(type_a: Typ, type_b: Typ) -> bool {
    type_a == type_b
}

/// Applies sub
pub fn sub(type_a: Typ, type_b: Typ) -> bool {
    matches!((type_a, type_b), (Typ::Nat, Typ::Int)) || equiv(type_a, type_b)
}

// Stringifiers

/// Renders num
pub fn string_of_num(number: &Number) -> String {
    match number {
        Number::Nat(natural) => natural.to_string(),
        Number::Int(integer) => {
            let sign = if integer.is_negative() { "-" } else { "+" };
            format!("{sign}{}", integer.abs())
        }
    }
}

/// Renders typ
pub fn string_of_typ(number_type: Typ) -> &'static str {
    match number_type {
        Typ::Nat => "nat",
        Typ::Int => "int",
    }
}

/// Renders unop
pub fn string_of_unop(unop: UnOp) -> &'static str {
    match unop {
        UnOp::Plus => "+",
        UnOp::Minus => "-",
    }
}

/// Renders binop
pub fn string_of_binop(binop: BinOp) -> &'static str {
    match binop {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "\\",
        BinOp::Pow => "^",
    }
}

/// Renders cmpop
pub fn string_of_cmpop(cmpop: CmpOp) -> &'static str {
    match cmpop {
        CmpOp::Lt => "<",
        CmpOp::Gt => ">",
        CmpOp::Le => "<=",
        CmpOp::Ge => ">=",
    }
}

// Unary

/// Applies un
pub fn un(unop: UnOp, number: &Number) -> Number {
    match unop {
        UnOp::Plus => number.clone(),
        UnOp::Minus => Number::Int(-to_int(number)),
    }
}

// Binary

/// Applies a checked binary operation
///
/// Returns an error for mismatched kinds;
/// returns an error for zero division or modulo
pub fn bin(binop: BinOp, number_l: &Number, number_r: &Number) -> Result<Number, NumericError> {
    match (binop, number_l, number_r) {
        (BinOp::Add, Number::Nat(natural_l), Number::Nat(natural_r)) => {
            Ok(Number::Nat(Natural(&natural_l.0 + &natural_r.0)))
        }
        (BinOp::Add, Number::Int(integer_l), Number::Int(integer_r)) => {
            Ok(Number::Int(integer_l + integer_r))
        }
        (BinOp::Sub, Number::Nat(natural_l), Number::Nat(natural_r)) => {
            Ok(Number::Int(&natural_l.0 - &natural_r.0))
        }
        (BinOp::Sub, Number::Int(integer_l), Number::Int(integer_r)) => {
            Ok(Number::Int(integer_l - integer_r))
        }
        (BinOp::Mul, Number::Nat(natural_l), Number::Nat(natural_r)) => {
            Ok(Number::Nat(Natural(&natural_l.0 * &natural_r.0)))
        }
        (BinOp::Mul, Number::Int(integer_l), Number::Int(integer_r)) => {
            Ok(Number::Int(integer_l * integer_r))
        }
        (binop @ (BinOp::Div | BinOp::Mod), Number::Nat(_), Number::Nat(natural_r))
            if natural_r.0.is_zero() =>
        {
            Err(NumericError::ZeroDivisor(binop))
        }
        (binop @ (BinOp::Div | BinOp::Mod), Number::Int(_), Number::Int(integer_r))
            if integer_r.is_zero() =>
        {
            Err(NumericError::ZeroDivisor(binop))
        }
        (BinOp::Div, Number::Nat(natural_l), Number::Nat(natural_r)) => {
            Ok(Number::Nat(Natural(&natural_l.0 / &natural_r.0)))
        }
        (BinOp::Div, Number::Int(integer_l), Number::Int(integer_r)) => {
            Ok(Number::Int(integer_l / integer_r))
        }
        (BinOp::Mod, Number::Nat(natural_l), Number::Nat(natural_r)) => {
            Ok(Number::Nat(Natural(&natural_l.0 % &natural_r.0)))
        }
        (BinOp::Mod, Number::Int(integer_l), Number::Int(integer_r)) => {
            Ok(Number::Int(integer_l % integer_r))
        }
        (BinOp::Pow, Number::Nat(_), Number::Nat(_))
        | (BinOp::Pow, Number::Int(_), Number::Int(_)) => {
            Err(NumericError::UnsupportedBinaryOperation(binop))
        }
        (_, number_l, number_r) => Err(NumericError::MismatchedKinds {
            typ_l: to_typ(number_l),
            typ_r: to_typ(number_r),
        }),
    }
}

// Comparison

/// Applies a checked comparison
///
/// Returns an error for mismatched number kinds
pub fn cmp(cmpop: CmpOp, number_l: &Number, number_r: &Number) -> Result<bool, NumericError> {
    match (cmpop, number_l, number_r) {
        (CmpOp::Lt, Number::Nat(natural_l), Number::Nat(natural_r)) => {
            Ok(natural_l.0 < natural_r.0)
        }
        (CmpOp::Lt, Number::Int(integer_l), Number::Int(integer_r)) => Ok(integer_l < integer_r),
        (CmpOp::Gt, Number::Nat(natural_l), Number::Nat(natural_r)) => {
            Ok(natural_l.0 > natural_r.0)
        }
        (CmpOp::Gt, Number::Int(integer_l), Number::Int(integer_r)) => Ok(integer_l > integer_r),
        (CmpOp::Le, Number::Nat(natural_l), Number::Nat(natural_r)) => {
            Ok(natural_l.0 <= natural_r.0)
        }
        (CmpOp::Le, Number::Int(integer_l), Number::Int(integer_r)) => Ok(integer_l <= integer_r),
        (CmpOp::Ge, Number::Nat(natural_l), Number::Nat(natural_r)) => {
            Ok(natural_l.0 >= natural_r.0)
        }
        (CmpOp::Ge, Number::Int(integer_l), Number::Int(integer_r)) => Ok(integer_l >= integer_r),
        (_, number_l, number_r) => Err(NumericError::MismatchedKinds {
            typ_l: to_typ(number_l),
            typ_r: to_typ(number_r),
        }),
    }
}
