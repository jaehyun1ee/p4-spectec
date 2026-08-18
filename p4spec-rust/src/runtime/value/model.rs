use std::rc::Rc;

use thiserror::Error;

use crate::{
    domain::{
        external_data::ExternalData,
        mixfix::Mixfix,
        source::{HasSpan, Span},
    },
    lang::{
        il::ast::{Atom, Id, TParam, Typ, TypKind},
        xl::num,
    },
};

// Value

pub type ValueRef = Rc<Value>;
pub type ValueField = (Atom, ValueRef);
pub type ValueCase = Mixfix<ValueRef>;

#[derive(Debug)]
pub struct Value {
    pub kind: ValueKind,
    pub ty: TypKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ValueKind {
    BoolV(bool),
    NumV(num::T),
    TextV(String),
    StructV(Vec<ValueField>),
    CaseV(ValueCase),
    TupleV(Vec<ValueRef>),
    OptV(Option<ValueRef>),
    ListV(Vec<ValueRef>),
    FuncV(Id),
    ExternV(ExternalData),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueTag {
    Bool,
    Num,
    Text,
    Struct,
    Case,
    Tuple,
    Opt,
    List,
    Func,
    Extern,
}

impl ValueKind {
    pub fn tag(&self) -> ValueTag {
        match self {
            Self::BoolV(_) => ValueTag::Bool,
            Self::NumV(_) => ValueTag::Num,
            Self::TextV(_) => ValueTag::Text,
            Self::StructV(_) => ValueTag::Struct,
            Self::CaseV(_) => ValueTag::Case,
            Self::TupleV(_) => ValueTag::Tuple,
            Self::OptV(_) => ValueTag::Opt,
            Self::ListV(_) => ValueTag::List,
            Self::FuncV(_) => ValueTag::Func,
            Self::ExternV(_) => ValueTag::Extern,
        }
    }
}

impl HasSpan for Value {
    fn span(&self) -> &Span {
        &self.span
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ValueError {
    #[error("expected {expected:?} value, got {actual:?}")]
    UnexpectedKind {
        expected: ValueTag,
        actual: ValueTag,
    },
    #[error("value index {index} is out of bounds for length {len}")]
    IndexOutOfBounds { index: usize, len: usize },
    #[error("expected exactly {expected} values, got {actual}")]
    ExpectedCount { expected: usize, actual: usize },
}

// Constructors

pub mod make {
    use super::*;
    use crate::runtime::r#type::typ::make as make_type;

    pub fn new(kind: ValueKind, ty: TypKind, span: Span) -> ValueRef {
        Rc::new(Value { kind, ty, span })
    }

    pub fn bool(value: bool, span: Span) -> ValueRef {
        new(ValueKind::BoolV(value), make_type::bool_kind(), span)
    }

    pub fn nat(value: num_bigint::BigInt, span: Span) -> ValueRef {
        new(
            ValueKind::NumV(num::T::Nat(value)),
            make_type::nat_kind(),
            span,
        )
    }

    pub fn int(value: num_bigint::BigInt, span: Span) -> ValueRef {
        new(
            ValueKind::NumV(num::T::Int(value)),
            make_type::int_kind(),
            span,
        )
    }

    pub fn num(value: num::T, span: Span) -> ValueRef {
        match value {
            num::T::Nat(value) => nat(value, span),
            num::T::Int(value) => int(value, span),
        }
    }

    pub fn text(value: String, span: Span) -> ValueRef {
        new(ValueKind::TextV(value), make_type::text_kind(), span)
    }

    pub fn structure(typ: &Typ, fields: Vec<ValueField>, span: Span) -> ValueRef {
        new(ValueKind::StructV(fields), typ.node.clone(), span)
    }

    pub fn case(typ: &Typ, value_case: ValueCase, span: Span) -> ValueRef {
        new(ValueKind::CaseV(value_case), typ.node.clone(), span)
    }

    pub fn tuple(typ: &Typ, values: Vec<ValueRef>, span: Span) -> ValueRef {
        new(ValueKind::TupleV(values), typ.node.clone(), span)
    }

    pub fn opt(typ: &Typ, value: Option<ValueRef>, span: Span) -> ValueRef {
        new(ValueKind::OptV(value), typ.node.clone(), span)
    }

    pub fn list(typ: &Typ, values: Vec<ValueRef>, span: Span) -> ValueRef {
        new(ValueKind::ListV(values), typ.node.clone(), span)
    }

    pub fn func(
        id: Id,
        type_params: Vec<TParam>,
        param_types: Vec<Typ>,
        return_type: Typ,
        span: Span,
    ) -> ValueRef {
        let ty = make_type::func_type(type_params, param_types, return_type).node;
        new(ValueKind::FuncV(id), ty, span)
    }

    pub fn external(typ: &Typ, value: ExternalData, span: Span) -> ValueRef {
        new(ValueKind::ExternV(value), typ.node.clone(), span)
    }
}

// Getters

pub mod get {
    use super::*;

    fn unexpected(value: &Value, expected: ValueTag) -> ValueError {
        ValueError::UnexpectedKind {
            expected,
            actual: value.kind.tag(),
        }
    }

    pub fn bool(value: &Value) -> Result<bool, ValueError> {
        match &value.kind {
            ValueKind::BoolV(value) => Ok(*value),
            _ => Err(unexpected(value, ValueTag::Bool)),
        }
    }

    pub fn num(value: &Value) -> Result<&num::T, ValueError> {
        match &value.kind {
            ValueKind::NumV(value) => Ok(value),
            _ => Err(unexpected(value, ValueTag::Num)),
        }
    }

    pub fn text(value: &Value) -> Result<&str, ValueError> {
        match &value.kind {
            ValueKind::TextV(value) => Ok(value),
            _ => Err(unexpected(value, ValueTag::Text)),
        }
    }

    pub fn structure(value: &Value) -> Result<&[ValueField], ValueError> {
        match &value.kind {
            ValueKind::StructV(fields) => Ok(fields),
            _ => Err(unexpected(value, ValueTag::Struct)),
        }
    }

    pub fn case(value: &Value) -> Result<&ValueCase, ValueError> {
        match &value.kind {
            ValueKind::CaseV(value_case) => Ok(value_case),
            _ => Err(unexpected(value, ValueTag::Case)),
        }
    }

    pub fn tuple(value: &Value) -> Result<&[ValueRef], ValueError> {
        match &value.kind {
            ValueKind::TupleV(values) => Ok(values),
            _ => Err(unexpected(value, ValueTag::Tuple)),
        }
    }

    pub fn opt(value: &Value) -> Result<Option<&ValueRef>, ValueError> {
        match &value.kind {
            ValueKind::OptV(value) => Ok(value.as_ref()),
            _ => Err(unexpected(value, ValueTag::Opt)),
        }
    }

    pub fn list(value: &Value) -> Result<&[ValueRef], ValueError> {
        match &value.kind {
            ValueKind::ListV(values) => Ok(values),
            _ => Err(unexpected(value, ValueTag::List)),
        }
    }

    pub fn func(value: &Value) -> Result<&Id, ValueError> {
        match &value.kind {
            ValueKind::FuncV(id) => Ok(id),
            _ => Err(unexpected(value, ValueTag::Func)),
        }
    }

    pub fn external(value: &Value) -> Result<&ExternalData, ValueError> {
        match &value.kind {
            ValueKind::ExternV(value) => Ok(value),
            _ => Err(unexpected(value, ValueTag::Extern)),
        }
    }

    pub fn nth(values: &[ValueRef], index: usize) -> Result<&ValueRef, ValueError> {
        values.get(index).ok_or(ValueError::IndexOutOfBounds {
            index,
            len: values.len(),
        })
    }

    pub fn one(values: &[ValueRef]) -> Result<&ValueRef, ValueError> {
        match values {
            [value] => Ok(value),
            _ => Err(ValueError::ExpectedCount {
                expected: 1,
                actual: values.len(),
            }),
        }
    }

    pub fn two(values: &[ValueRef]) -> Result<(&ValueRef, &ValueRef), ValueError> {
        match values {
            [value_a, value_b] => Ok((value_a, value_b)),
            _ => Err(ValueError::ExpectedCount {
                expected: 2,
                actual: values.len(),
            }),
        }
    }

    pub fn three(values: &[ValueRef]) -> Result<(&ValueRef, &ValueRef, &ValueRef), ValueError> {
        match values {
            [value_a, value_b, value_c] => Ok((value_a, value_b, value_c)),
            _ => Err(ValueError::ExpectedCount {
                expected: 3,
                actual: values.len(),
            }),
        }
    }
}
