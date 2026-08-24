//! Booleans

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum T {
    BoolT,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Typ {
    BoolT,
}

// Operations

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnOp {
    NotOp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinOp {
    AndOp,
    OrOp,
    ImplOp,
    EquivOp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CmpOp {
    EqOp,
    NeOp,
}

// Stringifiers

pub fn string_of_bool(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

pub fn string_of_unop(operation: UnOp) -> &'static str {
    match operation {
        UnOp::NotOp => "~",
    }
}

pub fn string_of_binop(operation: BinOp) -> &'static str {
    match operation {
        BinOp::AndOp => "/\\",
        BinOp::OrOp => "\\/",
        BinOp::ImplOp => "=>",
        BinOp::EquivOp => "<=>",
    }
}

pub fn string_of_cmpop(operation: CmpOp) -> &'static str {
    match operation {
        CmpOp::EqOp => "=",
        CmpOp::NeOp => "=/=",
    }
}
