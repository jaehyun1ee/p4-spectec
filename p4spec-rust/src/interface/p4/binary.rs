//! Binary-expression folding for the source parser's operator precedences
//!
//! The source lexer emits `>>` as two tokens. Menhir compares the leading
//! token at comparison precedence, then assigns shift precedence to the
//! completed operator. Other operators use one precedence in both positions.

use super::error::P4Error;

use crate::lang::{
    common::source::Span,
    data::value::{Value, ValueArena, make},
};

#[derive(Clone, Copy)]
pub(crate) enum BinaryOperator {
    LessEqual,
    GreaterEqual,
    Less,
    Greater,
    BitOr,
    BitXor,
    BitAnd,
    ShiftLeft,
    ShiftRight,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Precedence {
    Comparison,
    BitOr,
    BitXor,
    BitAnd,
    Shift,
}

pub(crate) struct BinaryExpressionPart {
    op: BinaryOperator,
    span: Span,
    value_r: Value,
}

impl BinaryExpressionPart {
    pub(crate) fn new(op: BinaryOperator, span: Span, value_r: Value) -> Self {
        Self { op, span, value_r }
    }
}

struct StackedOperator {
    op: BinaryOperator,
    precedence: Precedence,
    span: Span,
}

impl BinaryOperator {
    fn shape(self) -> &'static str {
        match self {
            Self::LessEqual => "'<='",
            Self::GreaterEqual => "'>='",
            Self::Less => "'<'",
            Self::Greater => "'>'",
            Self::BitOr => "'|'",
            Self::BitXor => "'^'",
            Self::BitAnd => "'&'",
            Self::ShiftLeft => "'<<'",
            Self::ShiftRight => "'>>'",
        }
    }

    fn precedence(self) -> Precedence {
        match self {
            Self::LessEqual | Self::GreaterEqual | Self::Less | Self::Greater => {
                Precedence::Comparison
            }
            Self::BitOr => Precedence::BitOr,
            Self::BitXor => Precedence::BitXor,
            Self::BitAnd => Precedence::BitAnd,
            Self::ShiftLeft | Self::ShiftRight => Precedence::Shift,
        }
    }

    fn incoming_precedence(self) -> Precedence {
        if matches!(self, Self::ShiftRight) {
            Precedence::Comparison
        } else {
            self.precedence()
        }
    }
}

fn reduce(
    arena: &mut ValueArena,
    values: &mut Vec<Value>,
    operators: &mut Vec<StackedOperator>,
) -> Result<(), P4Error> {
    let op = operators.pop().expect("binary operator");
    let value_r = values.pop().expect("binary right operand");
    let value_l = values.pop().expect("binary left operand");
    let value_operator = make::case_shaped! { arena: arena,
        shape: op.op.shape(),
        args: vec![],
        typ: "binop",
        span: op.span,
    }?;
    let span = Span::new(
        arena.span(&value_l).left.clone(),
        arena.span(&value_r).right.clone(),
    );
    values.push(make::case_shaped! { arena: arena,
        shape: "expression binop expression",
        args: vec![value_l, value_operator, value_r],
        typ: "binaryExpression",
        span: span,
    }?);
    Ok(())
}

pub(crate) fn fold(
    arena: &mut ValueArena,
    first: Value,
    parts: Vec<BinaryExpressionPart>,
) -> Result<Value, P4Error> {
    let mut values = vec![first];
    let mut operators: Vec<StackedOperator> = Vec::new();

    for part in parts {
        while operators
            .last()
            .is_some_and(|op| op.precedence >= part.op.incoming_precedence())
        {
            reduce(arena, &mut values, &mut operators)?;
        }
        operators.push(StackedOperator {
            op: part.op,
            precedence: part.op.precedence(),
            span: part.span,
        });
        values.push(part.value_r);
    }
    while !operators.is_empty() {
        reduce(arena, &mut values, &mut operators)?;
    }
    Ok(values.pop().expect("binary expression"))
}
