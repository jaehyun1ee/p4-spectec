// Booleans

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
