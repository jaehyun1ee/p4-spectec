//! Immutable values shared by language representations and runtimes
//!
//! A value carries its runtime type and source span through `NotePhrase`.
//! Smart constructors preserve that annotation, while projections expose each
//! payload with a typed error. Values also supply the total ordering and hash
//! required by runtime caches and collection-valued builtins.

use std::{
    cmp::Ordering,
    hash::{Hash, Hasher},
    rc::Rc,
};

use thiserror::Error;

use crate::{
    lang::{
        common::{
            Id, TId,
            notation::{atom, mixfix::Mixfix},
            source::{NotePhrase, Phrase, Span},
        },
        data::typ::{self, Typ, TypKind},
        xl::num::{self, Number},
    },
    yojson::ExternalData,
};

pub type Value = NotePhrase<ValueKind, TypKind>;

#[derive(Clone, Debug)]
pub enum ValueKind {
    Bool(bool),
    Num(Number),
    Text(String),
    Struct(Vec<ValueField>),
    Case(ValueCase),
    Tuple(Vec<Rc<Value>>),
    Opt(Option<Rc<Value>>),
    List(Vec<Rc<Value>>),
    Func(Id),
    Extern(ExternalData),
}

pub type ValueField = (Phrase<atom::Atom>, Rc<Value>);
pub type ValueCase = Mixfix<Rc<Value>>;

// == Comparison

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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
    fn tag(&self) -> ValueTag {
        match self {
            Self::Bool(_) => ValueTag::Bool,
            Self::Num(_) => ValueTag::Num,
            Self::Text(_) => ValueTag::Text,
            Self::Struct(_) => ValueTag::Struct,
            Self::Case(_) => ValueTag::Case,
            Self::Tuple(_) => ValueTag::Tuple,
            Self::Opt(_) => ValueTag::Opt,
            Self::List(_) => ValueTag::List,
            Self::Func(_) => ValueTag::Func,
            Self::Extern(_) => ValueTag::Extern,
        }
    }
}

impl Ord for ValueKind {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Bool(value_l), Self::Bool(value_r)) => value_l.cmp(value_r),
            (Self::Num(value_l), Self::Num(value_r)) => num::compare(value_l, value_r),
            (Self::Text(value_l), Self::Text(value_r)) => value_l.cmp(value_r),
            (Self::Struct(fields_l), Self::Struct(fields_r)) => {
                let keys_l = fields_l.iter().map(|(atom, value)| (&atom.node, value));
                let keys_r = fields_r.iter().map(|(atom, value)| (&atom.node, value));
                keys_l.cmp(keys_r)
            }
            (Self::Case(case_l), Self::Case(case_r)) => case_l.cmp(case_r),
            (Self::Tuple(values_l), Self::Tuple(values_r))
            | (Self::List(values_l), Self::List(values_r)) => values_l.cmp(values_r),
            (Self::Opt(value_l), Self::Opt(value_r)) => value_l.cmp(value_r),
            (Self::Func(id_l), Self::Func(id_r)) => id_l.node.cmp(&id_r.node),
            (Self::Extern(value_l), Self::Extern(value_r)) => value_l.cmp(value_r),
            _ => self.tag().cmp(&other.tag()),
        }
    }
}

impl PartialOrd for ValueKind {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ValueKind {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for ValueKind {}

// == Hashing

impl Hash for ValueKind {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.tag().hash(state);
        match self {
            Self::Bool(value) => value.hash(state),
            Self::Num(value) => value.hash(state),
            Self::Text(value) => value.hash(state),
            Self::Struct(fields) => {
                fields.len().hash(state);
                for (atom, value) in fields {
                    atom.node.hash(state);
                    value.hash(state);
                }
            }
            Self::Case(value_case) => value_case.hash(state),
            Self::Tuple(values) | Self::List(values) => values.hash(state),
            Self::Opt(value) => value.hash(state),
            Self::Func(id) => id.node.hash(state),
            Self::Extern(value) => value.hash(state),
        }
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

// == Constructors

pub mod make {
    use super::*;

    pub fn new(kind: ValueKind, typ: TypKind, span: Span) -> Rc<Value> {
        Rc::new(crate::note_phrase!(node: kind, note: typ, span: span))
    }

    pub fn bool(value: bool, span: Span) -> Rc<Value> {
        let kind = ValueKind::Bool(value);
        let typ = typ::make::bool().node;
        new(kind, typ, span)
    }

    pub fn nat(value: num::Natural, span: Span) -> Rc<Value> {
        let number = Number::Nat(value);
        let kind = ValueKind::Num(number);
        let typ = typ::make::nat().node;
        new(kind, typ, span)
    }

    pub fn int(value: num_bigint::BigInt, span: Span) -> Rc<Value> {
        let number = Number::Int(value);
        let kind = ValueKind::Num(number);
        let typ = typ::make::int().node;
        new(kind, typ, span)
    }

    pub fn num(value: Number, span: Span) -> Rc<Value> {
        match value {
            Number::Nat(value) => nat(value, span),
            Number::Int(value) => int(value, span),
        }
    }

    pub fn text(value: String, span: Span) -> Rc<Value> {
        let kind = ValueKind::Text(value);
        let typ = typ::make::text().node;
        new(kind, typ, span)
    }

    pub fn structure(typ: &Typ, fields: Vec<ValueField>, span: Span) -> Rc<Value> {
        let kind = ValueKind::Struct(fields);
        let typ = typ.node.clone();
        new(kind, typ, span)
    }

    pub fn case(typ: &Typ, value_case: ValueCase, span: Span) -> Rc<Value> {
        let kind = ValueKind::Case(value_case);
        let typ = typ.node.clone();
        new(kind, typ, span)
    }

    pub fn tuple(typ: &Typ, values: Vec<Rc<Value>>, span: Span) -> Rc<Value> {
        let kind = ValueKind::Tuple(values);
        let typ = typ.node.clone();
        new(kind, typ, span)
    }

    pub fn opt(typ: &Typ, value: Option<Rc<Value>>, span: Span) -> Rc<Value> {
        let kind = ValueKind::Opt(value);
        let typ = typ.node.clone();
        new(kind, typ, span)
    }

    pub fn list(typ: &Typ, values: Vec<Rc<Value>>, span: Span) -> Rc<Value> {
        let kind = ValueKind::List(values);
        let typ = typ.node.clone();
        new(kind, typ, span)
    }

    pub fn func(
        id: Id,
        tparams: Vec<TId>,
        typs_params: Vec<Typ>,
        typ_ret: Typ,
        span: Span,
    ) -> Rc<Value> {
        let typ = typ::make::func(tparams, typs_params, typ_ret).node;
        let kind = ValueKind::Func(id);
        new(kind, typ, span)
    }

    pub fn external(typ: &Typ, value: ExternalData, span: Span) -> Rc<Value> {
        let kind = ValueKind::Extern(value);
        let typ = typ.node.clone();
        new(kind, typ, span)
    }

    pub fn retag(value: Rc<Value>, typ: &Typ) -> Rc<Value> {
        match Rc::try_unwrap(value) {
            Ok(mut value) => {
                value.note = typ.node.clone();
                Rc::new(value)
            }
            Err(value) => new(value.node.clone(), typ.node.clone(), value.span.clone()),
        }
    }
}

// == Projections

pub mod get {
    use super::*;

    fn unexpected(value: &Value, expected: ValueTag) -> ValueError {
        ValueError::UnexpectedKind {
            expected,
            actual: value.node.tag(),
        }
    }

    pub fn bool(value: &Value) -> Result<bool, ValueError> {
        match &value.node {
            ValueKind::Bool(value) => Ok(*value),
            _ => Err(unexpected(value, ValueTag::Bool)),
        }
    }

    pub fn num(value: &Value) -> Result<&Number, ValueError> {
        match &value.node {
            ValueKind::Num(value) => Ok(value),
            _ => Err(unexpected(value, ValueTag::Num)),
        }
    }

    pub fn text(value: &Value) -> Result<&str, ValueError> {
        match &value.node {
            ValueKind::Text(value) => Ok(value),
            _ => Err(unexpected(value, ValueTag::Text)),
        }
    }

    pub fn structure(value: &Value) -> Result<&[ValueField], ValueError> {
        match &value.node {
            ValueKind::Struct(fields) => Ok(fields),
            _ => Err(unexpected(value, ValueTag::Struct)),
        }
    }

    pub fn case(value: &Value) -> Result<&ValueCase, ValueError> {
        match &value.node {
            ValueKind::Case(value_case) => Ok(value_case),
            _ => Err(unexpected(value, ValueTag::Case)),
        }
    }

    pub fn tuple(value: &Value) -> Result<&[Rc<Value>], ValueError> {
        match &value.node {
            ValueKind::Tuple(values) => Ok(values),
            _ => Err(unexpected(value, ValueTag::Tuple)),
        }
    }

    pub fn opt(value: &Value) -> Result<Option<&Rc<Value>>, ValueError> {
        match &value.node {
            ValueKind::Opt(value) => Ok(value.as_ref()),
            _ => Err(unexpected(value, ValueTag::Opt)),
        }
    }

    pub fn list(value: &Value) -> Result<&[Rc<Value>], ValueError> {
        match &value.node {
            ValueKind::List(values) => Ok(values),
            _ => Err(unexpected(value, ValueTag::List)),
        }
    }

    pub fn func(value: &Value) -> Result<&Id, ValueError> {
        match &value.node {
            ValueKind::Func(id) => Ok(id),
            _ => Err(unexpected(value, ValueTag::Func)),
        }
    }

    pub fn external(value: &Value) -> Result<&ExternalData, ValueError> {
        match &value.node {
            ValueKind::Extern(value) => Ok(value),
            _ => Err(unexpected(value, ValueTag::Extern)),
        }
    }

    pub fn nth(values: &[Rc<Value>], index: usize) -> Result<&Rc<Value>, ValueError> {
        values.get(index).ok_or(ValueError::IndexOutOfBounds {
            index,
            len: values.len(),
        })
    }

    pub fn one(values: &[Rc<Value>]) -> Result<&Rc<Value>, ValueError> {
        match values {
            [value] => Ok(value),
            _ => Err(ValueError::ExpectedCount {
                expected: 1,
                actual: values.len(),
            }),
        }
    }

    pub fn two(values: &[Rc<Value>]) -> Result<(&Rc<Value>, &Rc<Value>), ValueError> {
        match values {
            [value_a, value_b] => Ok((value_a, value_b)),
            _ => Err(ValueError::ExpectedCount {
                expected: 2,
                actual: values.len(),
            }),
        }
    }

    #[allow(clippy::type_complexity)]
    pub fn three(values: &[Rc<Value>]) -> Result<(&Rc<Value>, &Rc<Value>, &Rc<Value>), ValueError> {
        match values {
            [value_a, value_b, value_c] => Ok((value_a, value_b, value_c)),
            _ => Err(ValueError::ExpectedCount {
                expected: 3,
                actual: values.len(),
            }),
        }
    }
}
