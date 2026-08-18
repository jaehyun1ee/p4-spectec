use num_bigint::BigInt;

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

// Subtyping

pub fn equiv(type_a: Typ, type_b: Typ) -> bool {
    type_a == type_b
}

pub fn sub(type_a: Typ, type_b: Typ) -> bool {
    matches!((type_a, type_b), (Typ::NatT, Typ::IntT)) || equiv(type_a, type_b)
}
