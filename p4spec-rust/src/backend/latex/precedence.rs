//! Parenthesization of nested EL expressions
//!
//! Categories increase from implication to atomic syntax.
//! Equal categories use associativity and operand side,
//! preserving distinctions such as `a - (b - c)`.

use crate::lang::{
    common::{
        notation::atom::Atom,
        prim::{bool, num},
    },
    el::ast::BinOp,
};

// == Precedence model

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Category {
    Implication,
    Disjunction,
    Conjunction,
    Turnstile,
    Tilesturn,
    SquigglyArrow,
    Colon,
    Comparison,
    Cons,
    Arrow,
    Semicolon,
    Dot,
    Additive,
    Multiplicative,
    Unary,
    Sequence,
    Power,
    Postfix,
    Atomic,
}

#[derive(Clone, Copy)]
pub(super) enum Assoc {
    Left,
    Right,
    Non,
}

#[derive(Clone, Copy)]
pub(super) enum Side {
    Left,
    Right,
}

// == Operator categories

// - Notation atoms

/// Returns the binding category and associativity of a notation atom.
pub(super) fn of_infix(atom: &Atom) -> (Category, Assoc) {
    use Assoc as A;
    use Category as C;
    match atom {
        Atom::DoubleArrowSub | Atom::DoubleArrowLong => (C::Implication, A::Right),
        Atom::Turnstile => (C::Turnstile, A::Non),
        Atom::Tilesturn => (C::Tilesturn, A::Non),
        Atom::SqArrow | Atom::SqArrowStar => (C::SquigglyArrow, A::Right),
        Atom::Sub | Atom::Sup | Atom::Colon | Atom::ColonEq | Atom::Tilde2 => (C::Colon, A::Left),
        Atom::Arrow | Atom::ArrowSub => (C::Arrow, A::Right),
        Atom::Semicolon => (C::Semicolon, A::Left),
        Atom::Dot | Atom::Dot2 | Atom::Dot3 => (C::Dot, A::Left),
        Atom::Backslash => (C::Multiplicative, A::Left),
        Atom::Keyword(_)
        | Atom::Tag(_)
        | Atom::Operator(_)
        | Atom::LAngle
        | Atom::RAngle
        | Atom::LParen
        | Atom::RParen
        | Atom::LBrack
        | Atom::RBrack
        | Atom::LBrace
        | Atom::RBrace => (C::Colon, A::Non),
    }
}

// - Binary operators

/// Returns the binding category and associativity of a binary operator.
pub(super) fn of_binop(op: BinOp) -> (Category, Assoc) {
    use Assoc as A;
    use Category as C;
    match op {
        BinOp::Bool(bool::BinOp::Impl | bool::BinOp::Equiv) => (C::Implication, A::Right),
        BinOp::Bool(bool::BinOp::Or) => (C::Disjunction, A::Left),
        BinOp::Bool(bool::BinOp::And) => (C::Conjunction, A::Left),
        BinOp::Num(num::BinOp::Add | num::BinOp::Sub) => (C::Additive, A::Left),
        BinOp::Num(num::BinOp::Mul | num::BinOp::Div | num::BinOp::Mod) => {
            (C::Multiplicative, A::Left)
        }
        BinOp::Num(num::BinOp::Pow) => (C::Power, A::Left),
    }
}

// == Parenthesization

/// Determines whether an operand needs parentheses under its parent operator.
pub(super) fn needs_parentheses(
    category_parent: Category,
    assoc: Assoc,
    side: Side,
    category_child: Category,
) -> bool {
    category_child < category_parent
        || category_child == category_parent
            && matches!(
                (assoc, side),
                (Assoc::Left, Side::Right) | (Assoc::Right, Side::Left) | (Assoc::Non, _)
            )
}
