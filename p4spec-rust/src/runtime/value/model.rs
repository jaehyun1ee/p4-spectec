use std::{
    cmp::Ordering,
    fmt,
    hash::{DefaultHasher, Hash, Hasher},
    ops::Deref,
    rc::Rc,
    sync::OnceLock,
};

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValueSpan(Option<Rc<Span>>);

impl ValueSpan {
    fn new(span: Span) -> Self {
        if span == Span::default() {
            Self(None)
        } else {
            Self(Some(Rc::new(span)))
        }
    }

    pub fn region(&self) -> &Span {
        static NONE: OnceLock<Span> = OnceLock::new();
        self.0
            .as_deref()
            .unwrap_or_else(|| NONE.get_or_init(Span::default))
    }
}

impl Deref for ValueSpan {
    type Target = Span;

    fn deref(&self) -> &Self::Target {
        self.region()
    }
}

impl PartialEq<Span> for ValueSpan {
    fn eq(&self, other: &Span) -> bool {
        self.region() == other
    }
}

impl PartialEq<ValueSpan> for Span {
    fn eq(&self, other: &ValueSpan) -> bool {
        self == other.region()
    }
}

impl fmt::Display for ValueSpan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.region().fmt(formatter)
    }
}

#[derive(Debug)]
pub struct Value {
    pub kind: ValueKind,
    pub ty: TypKind,
    pub span: ValueSpan,
    semantic_hash: u64,
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

// Comparison

fn kind_rank(kind: &ValueKind) -> u8 {
    match kind {
        ValueKind::BoolV(_) => 0,
        ValueKind::NumV(_) => 1,
        ValueKind::TextV(_) => 2,
        ValueKind::StructV(_) => 3,
        ValueKind::CaseV(_) => 4,
        ValueKind::TupleV(_) => 5,
        ValueKind::OptV(None) => 6,
        ValueKind::OptV(Some(_)) => 7,
        ValueKind::ListV(_) => 8,
        ValueKind::FuncV(_) => 9,
        ValueKind::ExternV(_) => 10,
    }
}

fn compare_num(num_l: &num::T, num_r: &num::T) -> Ordering {
    match (num_l, num_r) {
        (num::T::Nat(num_l), num::T::Nat(num_r)) => num_l.cmp(num_r),
        (num::T::Int(num_l), num::T::Int(num_r)) => num_l.cmp(num_r),
        (num::T::Nat(_), num::T::Int(_)) => Ordering::Less,
        (num::T::Int(_), num::T::Nat(_)) => Ordering::Greater,
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
        (false, false) if float_l == float_r => Ordering::Equal,
        (false, false) => float_l
            .partial_cmp(&float_r)
            .expect("non-NaN floats are comparable"),
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
            (ValueKind::BoolV(value_l), ValueKind::BoolV(value_r)) => value_l.cmp(value_r),
            (ValueKind::NumV(value_l), ValueKind::NumV(value_r)) => compare_num(value_l, value_r),
            (ValueKind::TextV(value_l), ValueKind::TextV(value_r)) => value_l.cmp(value_r),
            (ValueKind::StructV(fields_l), ValueKind::StructV(fields_r)) => {
                compare_fields(fields_l, fields_r)
            }
            (ValueKind::CaseV(case_l), ValueKind::CaseV(case_r)) => case_l.cmp(case_r),
            (ValueKind::TupleV(values_l), ValueKind::TupleV(values_r))
            | (ValueKind::ListV(values_l), ValueKind::ListV(values_r)) => values_l.cmp(values_r),
            (ValueKind::OptV(value_l), ValueKind::OptV(value_r)) => value_l.cmp(value_r),
            (ValueKind::FuncV(id_l), ValueKind::FuncV(id_r)) => id_l.node.cmp(&id_r.node),
            (ValueKind::ExternV(value_l), ValueKind::ExternV(value_r)) => {
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

// Equality

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Value {}

// Hash computation

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
        ValueKind::BoolV(value) => value.hash(state),
        ValueKind::NumV(num::T::Nat(value)) => {
            0_u8.hash(state);
            value.hash(state);
        }
        ValueKind::NumV(num::T::Int(value)) => {
            1_u8.hash(state);
            value.hash(state);
        }
        ValueKind::TextV(value) => value.hash(state),
        ValueKind::StructV(fields) => {
            fields.len().hash(state);
            for (atom, value) in fields {
                atom.node.hash(state);
                value.hash(state);
            }
        }
        ValueKind::CaseV(value_case) => value_case.hash(state),
        ValueKind::TupleV(values) | ValueKind::ListV(values) => values.hash(state),
        ValueKind::OptV(value) => value.hash(state),
        ValueKind::FuncV(id) => id.node.hash(state),
        ValueKind::ExternV(value) => hash_external(value, state),
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

impl HasSpan for Value {
    fn span(&self) -> &Span {
        self.span.region()
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
        let semantic_hash = semantic_hash(&kind);
        Rc::new(Value {
            kind,
            ty,
            span: ValueSpan::new(span),
            semantic_hash,
        })
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
        structure_kind(&typ.node, fields, span)
    }

    pub fn structure_kind(typ: &TypKind, fields: Vec<ValueField>, span: Span) -> ValueRef {
        new(ValueKind::StructV(fields), typ.clone(), span)
    }

    pub fn case(typ: &Typ, value_case: ValueCase, span: Span) -> ValueRef {
        case_kind(&typ.node, value_case, span)
    }

    pub fn case_kind(typ: &TypKind, value_case: ValueCase, span: Span) -> ValueRef {
        new(ValueKind::CaseV(value_case), typ.clone(), span)
    }

    pub fn tuple(typ: &Typ, values: Vec<ValueRef>, span: Span) -> ValueRef {
        tuple_kind(&typ.node, values, span)
    }

    pub fn tuple_kind(typ: &TypKind, values: Vec<ValueRef>, span: Span) -> ValueRef {
        new(ValueKind::TupleV(values), typ.clone(), span)
    }

    pub fn opt(typ: &Typ, value: Option<ValueRef>, span: Span) -> ValueRef {
        opt_kind(&typ.node, value, span)
    }

    pub fn opt_kind(typ: &TypKind, value: Option<ValueRef>, span: Span) -> ValueRef {
        new(ValueKind::OptV(value), typ.clone(), span)
    }

    pub fn list(typ: &Typ, values: Vec<ValueRef>, span: Span) -> ValueRef {
        list_kind(&typ.node, values, span)
    }

    pub fn list_kind(typ: &TypKind, values: Vec<ValueRef>, span: Span) -> ValueRef {
        new(ValueKind::ListV(values), typ.clone(), span)
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
