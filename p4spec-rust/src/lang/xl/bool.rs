//! Booleans

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Typ {
    Bool,
}

// Operations

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnOp {
    Not,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinOp {
    And,
    Or,
    Impl,
    Equiv,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CmpOp {
    Eq,
    Ne,
}

// Stringifiers

/// Renders bool
pub fn string_of_bool(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

/// Renders unop
pub fn string_of_unop(unop: UnOp) -> &'static str {
    match unop {
        UnOp::Not => "~",
    }
}

/// Renders binop
pub fn string_of_binop(binop: BinOp) -> &'static str {
    match binop {
        BinOp::And => "/\\",
        BinOp::Or => "\\/",
        BinOp::Impl => "=>",
        BinOp::Equiv => "<=>",
    }
}

/// Renders cmpop
pub fn string_of_cmpop(cmpop: CmpOp) -> &'static str {
    match cmpop {
        CmpOp::Eq => "=",
        CmpOp::Ne => "=/=",
    }
}
