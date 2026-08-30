use std::{
    cmp::Ordering,
    hash::{DefaultHasher, Hash, Hasher},
    rc::Rc,
};

use thiserror::Error;

use crate::{
    lang::{
        common::{notation::mixfix::Mixfix, source::Span},
        il::ast::{Atom, Id, TParam, Typ, TypKind},
        xl::num::{self, Number},
    },
    runtime::types::typ,
    yojson::ExternalData,
};

pub type ValueRef = Rc<Value>;
pub type ValueField = (Atom, ValueRef);
pub type ValueCase = Mixfix<ValueRef>;

#[derive(Debug)]
pub struct Value {
    pub kind: ValueKind,
    pub typ: TypKind,
    pub span: Span,
    semantic_hash: u64,
}

#[derive(Clone, Debug)]
pub enum ValueKind {
    Bool(bool),
    Num(Number),
    Text(String),
    Struct(Vec<ValueField>),
    Case(ValueCase),
    Tuple(Vec<ValueRef>),
    Opt(Option<ValueRef>),
    List(Vec<ValueRef>),
    Func(Id),
    Extern(ExternalData),
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

fn kind_rank(kind: &ValueKind) -> u8 {
    match kind {
        ValueKind::Bool(_) => 0,
        ValueKind::Num(_) => 1,
        ValueKind::Text(_) => 2,
        ValueKind::Struct(_) => 3,
        ValueKind::Case(_) => 4,
        ValueKind::Tuple(_) => 5,
        ValueKind::Opt(None) => 6,
        ValueKind::Opt(Some(_)) => 7,
        ValueKind::List(_) => 8,
        ValueKind::Func(_) => 9,
        ValueKind::Extern(_) => 10,
    }
}

fn compare_fields(fields_l: &[ValueField], fields_r: &[ValueField]) -> Ordering {
    for ((atom_l, value_l), (atom_r, value_r)) in fields_l.iter().zip(fields_r) {
        let ordering = atom_l.node.cmp(&atom_r.node);
        if ordering != Ordering::Equal {
            return ordering;
        }
        let ordering = value_l.cmp(value_r);
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    fields_l.len().cmp(&fields_r.len())
}

fn compare_float(float_l: f64, float_r: f64) -> Ordering {
    match (float_l.is_nan(), float_r.is_nan()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => float_l.total_cmp(&float_r),
    }
}

fn external_rank(value: &ExternalData) -> u8 {
    match value {
        ExternalData::Null => 0,
        ExternalData::String(_) => 1,
        ExternalData::Intlit(_) => 2,
        ExternalData::Int(_) => 3,
        ExternalData::Float(_) => 4,
        ExternalData::Variant(_, _) => 5,
        ExternalData::Tuple(_) => 6,
        ExternalData::Bool(_) => 7,
        ExternalData::List(_) => 8,
        ExternalData::Assoc(_) => 9,
    }
}

fn compare_external_slices(values_l: &[ExternalData], values_r: &[ExternalData]) -> Ordering {
    for (value_l, value_r) in values_l.iter().zip(values_r) {
        let ordering = compare_external(value_l, value_r);
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    values_l.len().cmp(&values_r.len())
}

fn compare_external_fields(
    fields_l: &[(String, ExternalData)],
    fields_r: &[(String, ExternalData)],
) -> Ordering {
    for ((name_l, value_l), (name_r, value_r)) in fields_l.iter().zip(fields_r) {
        let ordering = name_l.cmp(name_r);
        if ordering != Ordering::Equal {
            return ordering;
        }
        let ordering = compare_external(value_l, value_r);
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    fields_l.len().cmp(&fields_r.len())
}

fn compare_external(value_l: &ExternalData, value_r: &ExternalData) -> Ordering {
    match (value_l, value_r) {
        (ExternalData::Null, ExternalData::Null) => Ordering::Equal,
        (ExternalData::String(value_l), ExternalData::String(value_r))
        | (ExternalData::Intlit(value_l), ExternalData::Intlit(value_r)) => value_l.cmp(value_r),
        (ExternalData::Int(value_l), ExternalData::Int(value_r)) => value_l.cmp(value_r),
        (ExternalData::Float(value_l), ExternalData::Float(value_r)) => {
            compare_float(*value_l, *value_r)
        }
        (ExternalData::Variant(name_l, value_l), ExternalData::Variant(name_r, value_r)) => {
            name_l.cmp(name_r).then_with(|| match (value_l, value_r) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Less,
                (Some(_), None) => Ordering::Greater,
                (Some(value_l), Some(value_r)) => compare_external(value_l, value_r),
            })
        }
        (ExternalData::Tuple(values_l), ExternalData::Tuple(values_r))
        | (ExternalData::List(values_l), ExternalData::List(values_r)) => {
            compare_external_slices(values_l, values_r)
        }
        (ExternalData::Bool(value_l), ExternalData::Bool(value_r)) => value_l.cmp(value_r),
        (ExternalData::Assoc(fields_l), ExternalData::Assoc(fields_r)) => {
            compare_external_fields(fields_l, fields_r)
        }
        _ => external_rank(value_l).cmp(&external_rank(value_r)),
    }
}

impl Ord for Value {
    fn cmp(&self, other: &Self) -> Ordering {
        if std::ptr::eq(self, other) {
            return Ordering::Equal;
        }
        match (&self.kind, &other.kind) {
            (ValueKind::Bool(value_l), ValueKind::Bool(value_r)) => value_l.cmp(value_r),
            (ValueKind::Num(value_l), ValueKind::Num(value_r)) => num::compare(value_l, value_r),
            (ValueKind::Text(value_l), ValueKind::Text(value_r)) => value_l.cmp(value_r),
            (ValueKind::Struct(fields_l), ValueKind::Struct(fields_r)) => {
                compare_fields(fields_l, fields_r)
            }
            (ValueKind::Case(case_l), ValueKind::Case(case_r)) => case_l.cmp(case_r),
            (ValueKind::Tuple(values_l), ValueKind::Tuple(values_r))
            | (ValueKind::List(values_l), ValueKind::List(values_r)) => values_l.cmp(values_r),
            (ValueKind::Opt(value_l), ValueKind::Opt(value_r)) => value_l.cmp(value_r),
            (ValueKind::Func(id_l), ValueKind::Func(id_r)) => id_l.node.cmp(&id_r.node),
            (ValueKind::Extern(value_l), ValueKind::Extern(value_r)) => {
                compare_external(value_l, value_r)
            }
            _ => kind_rank(&self.kind).cmp(&kind_rank(&other.kind)),
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.semantic_hash == other.semantic_hash && self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Value {}

fn hash_external<H: Hasher>(value: &ExternalData, state: &mut H) {
    external_rank(value).hash(state);
    match value {
        ExternalData::Null => {}
        ExternalData::Bool(value) => value.hash(state),
        ExternalData::Int(value) => value.hash(state),
        ExternalData::Intlit(value) | ExternalData::String(value) => value.hash(state),
        ExternalData::Float(value) => {
            let bits = if value.is_nan() {
                f64::NAN.to_bits()
            } else if *value == 0.0 {
                0.0f64.to_bits()
            } else {
                value.to_bits()
            };
            bits.hash(state);
        }
        ExternalData::Assoc(fields) => {
            fields.len().hash(state);
            for (name, value) in fields {
                name.hash(state);
                hash_external(value, state);
            }
        }
        ExternalData::List(values) | ExternalData::Tuple(values) => {
            values.len().hash(state);
            for value in values {
                hash_external(value, state);
            }
        }
        ExternalData::Variant(name, value) => {
            name.hash(state);
            value.is_some().hash(state);
            if let Some(value) = value {
                hash_external(value, state);
            }
        }
    }
}

fn hash_kind<H: Hasher>(kind: &ValueKind, state: &mut H) {
    kind_rank(kind).hash(state);
    match kind {
        ValueKind::Bool(value) => value.hash(state),
        ValueKind::Num(value) => value.hash(state),
        ValueKind::Text(value) => value.hash(state),
        ValueKind::Struct(fields) => {
            fields.len().hash(state);
            for (atom, value) in fields {
                atom.node.hash(state);
                value.hash(state);
            }
        }
        ValueKind::Case(value_case) => value_case.hash(state),
        ValueKind::Tuple(values) | ValueKind::List(values) => values.hash(state),
        ValueKind::Opt(value) => value.hash(state),
        ValueKind::Func(id) => id.node.hash(state),
        ValueKind::Extern(value) => hash_external(value, state),
    }
}

fn semantic_hash(kind: &ValueKind) -> u64 {
    let mut state = DefaultHasher::new();
    hash_kind(kind, &mut state);
    state.finish()
}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.semantic_hash);
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

pub mod make {
    use super::*;

    pub fn new(kind: ValueKind, typ: TypKind, span: Span) -> ValueRef {
        let semantic_hash = semantic_hash(&kind);
        Rc::new(Value {
            kind,
            typ,
            span,
            semantic_hash,
        })
    }

    pub fn bool(value: bool, span: Span) -> ValueRef {
        new(ValueKind::Bool(value), typ::bool().node, span)
    }

    pub fn nat(value: num::Natural, span: Span) -> ValueRef {
        new(ValueKind::Num(Number::Nat(value)), typ::nat().node, span)
    }

    pub fn int(value: num_bigint::BigInt, span: Span) -> ValueRef {
        new(ValueKind::Num(Number::Int(value)), typ::int().node, span)
    }

    pub fn num(value: Number, span: Span) -> ValueRef {
        match value {
            Number::Nat(value) => nat(value, span),
            Number::Int(value) => int(value, span),
        }
    }

    pub fn text(value: String, span: Span) -> ValueRef {
        new(ValueKind::Text(value), typ::text().node, span)
    }

    pub fn structure(typ: &Typ, fields: Vec<ValueField>, span: Span) -> ValueRef {
        new(ValueKind::Struct(fields), typ.node.clone(), span)
    }

    pub fn case(typ: &Typ, value_case: ValueCase, span: Span) -> ValueRef {
        new(ValueKind::Case(value_case), typ.node.clone(), span)
    }

    pub fn tuple(typ: &Typ, values: Vec<ValueRef>, span: Span) -> ValueRef {
        new(ValueKind::Tuple(values), typ.node.clone(), span)
    }

    pub fn opt(typ: &Typ, value: Option<ValueRef>, span: Span) -> ValueRef {
        new(ValueKind::Opt(value), typ.node.clone(), span)
    }

    pub fn list(typ: &Typ, values: Vec<ValueRef>, span: Span) -> ValueRef {
        new(ValueKind::List(values), typ.node.clone(), span)
    }

    pub fn func(
        id: Id,
        tparams: Vec<TParam>,
        typs_params: Vec<Typ>,
        typ_ret: Typ,
        span: Span,
    ) -> ValueRef {
        let typ = typ::func(tparams, typs_params, typ_ret).node;
        new(ValueKind::Func(id), typ, span)
    }

    pub fn external(typ: &Typ, value: ExternalData, span: Span) -> ValueRef {
        new(ValueKind::Extern(value), typ.node.clone(), span)
    }
}

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
            ValueKind::Bool(value) => Ok(*value),
            _ => Err(unexpected(value, ValueTag::Bool)),
        }
    }

    pub fn num(value: &Value) -> Result<&Number, ValueError> {
        match &value.kind {
            ValueKind::Num(value) => Ok(value),
            _ => Err(unexpected(value, ValueTag::Num)),
        }
    }

    pub fn text(value: &Value) -> Result<&str, ValueError> {
        match &value.kind {
            ValueKind::Text(value) => Ok(value),
            _ => Err(unexpected(value, ValueTag::Text)),
        }
    }

    pub fn structure(value: &Value) -> Result<&[ValueField], ValueError> {
        match &value.kind {
            ValueKind::Struct(fields) => Ok(fields),
            _ => Err(unexpected(value, ValueTag::Struct)),
        }
    }

    pub fn case(value: &Value) -> Result<&ValueCase, ValueError> {
        match &value.kind {
            ValueKind::Case(value_case) => Ok(value_case),
            _ => Err(unexpected(value, ValueTag::Case)),
        }
    }

    pub fn tuple(value: &Value) -> Result<&[ValueRef], ValueError> {
        match &value.kind {
            ValueKind::Tuple(values) => Ok(values),
            _ => Err(unexpected(value, ValueTag::Tuple)),
        }
    }

    pub fn opt(value: &Value) -> Result<Option<&ValueRef>, ValueError> {
        match &value.kind {
            ValueKind::Opt(value) => Ok(value.as_ref()),
            _ => Err(unexpected(value, ValueTag::Opt)),
        }
    }

    pub fn list(value: &Value) -> Result<&[ValueRef], ValueError> {
        match &value.kind {
            ValueKind::List(values) => Ok(values),
            _ => Err(unexpected(value, ValueTag::List)),
        }
    }

    pub fn func(value: &Value) -> Result<&Id, ValueError> {
        match &value.kind {
            ValueKind::Func(id) => Ok(id),
            _ => Err(unexpected(value, ValueTag::Func)),
        }
    }

    pub fn external(value: &Value) -> Result<&ExternalData, ValueError> {
        match &value.kind {
            ValueKind::Extern(value) => Ok(value),
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
