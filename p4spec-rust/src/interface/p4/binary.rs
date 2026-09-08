//! Binary-expression folding for the source parser's operator precedences
//!
//! The source lexer emits `>>` as two tokens. Menhir compares the leading
//! token at comparison precedence, then assigns shift precedence to the
//! completed operator. Other operators use one precedence in both positions.

use std::rc::Rc;

use crate::lang::{
    common::source::Span,
    data::value::{Value, make},
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
    operator: BinaryOperator,
    span: Span,
    rhs: Rc<Value>,
}

impl BinaryExpressionPart {
    pub(crate) fn new(operator: BinaryOperator, span: Span, rhs: Rc<Value>) -> Self {
        Self {
            operator,
            span,
            rhs,
        }
    }
}

struct StackedOperator {
    operator: BinaryOperator,
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

fn reduce(values: &mut Vec<Rc<Value>>, operators: &mut Vec<StackedOperator>) {
    let operator = operators.pop().expect("binary operator");
    let rhs = values.pop().expect("binary right operand");
    let lhs = values.pop().expect("binary left operand");
    let value_operator = make::case! {
        shape: operator.operator.shape(),
        args: vec![],
        typ: "binop",
        span: operator.span,
    };
    let span = Span::new(lhs.span.left.clone(), rhs.span.right.clone());
    values.push(make::case! {
        shape: "expression binop expression",
        args: vec![lhs, value_operator, rhs],
        typ: "binaryExpression",
        span: span,
    });
}

pub(crate) fn fold(first: Rc<Value>, parts: Vec<BinaryExpressionPart>) -> Rc<Value> {
    let mut values = vec![first];
    let mut operators: Vec<StackedOperator> = Vec::new();

    for part in parts {
        while operators
            .last()
            .is_some_and(|operator| operator.precedence >= part.operator.incoming_precedence())
        {
            reduce(&mut values, &mut operators);
        }
        operators.push(StackedOperator {
            operator: part.operator,
            precedence: part.operator.precedence(),
            span: part.span,
        });
        values.push(part.rhs);
    }
    while !operators.is_empty() {
        reduce(&mut values, &mut operators);
    }
    values.pop().expect("binary expression")
}
