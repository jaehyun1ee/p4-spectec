//! Value bodies with full source locations and canonical syntax identities

use std::{
    cmp::Ordering,
    hash::{Hash, Hasher},
    num::TryFromIntError,
};

use super::{
    arena::ValueArena,
    intern::{CanonInterner, Interned},
};
use thiserror::Error;

use crate::{
    lang::{
        common::{
            Id,
            notation::{atom::Atom, mixfix::Mixfix},
            source::{NotePhrase, Phrase, Span},
        },
        data::typ::TypKind,
        traits::{cmp::SyntaxCmp, eq::SyntaxEq},
        xl::num::{self, Number},
    },
    yojson::ExternalData,
};

// = Value types

pub type Value = NotePhrase<Interned<ValueKind>, Interned<TypKind>, Interned<Span>>;
pub type ValueField = (Phrase<Atom>, Value);
pub type ValueCase = Mixfix<Value, Interned<Span>>;

// - Borrowed views

#[derive(Clone, Copy, Debug)]
pub struct ValueRef<'a> {
    pub(super) arena: &'a ValueArena,
    pub(super) value: Value,
}

// - Bodies

#[derive(Debug)]
pub enum ValueKind {
    Bool(bool),
    Num(Number),
    Text(String),
    Struct(Vec<ValueField>),
    Case(ValueCase),
    Tuple(Vec<Value>),
    Opt(Option<Value>),
    List(Vec<Value>),
    Func(Id),
    Extern(ExternalData),
}

// - Tags

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
    pub(super) fn tag(&self) -> ValueTag {
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

// = Exact body equality and hashing

impl PartialEq for ValueKind {
    fn eq(&self, kind_other: &Self) -> bool {
        match (self, kind_other) {
            (Self::Bool(value_l), Self::Bool(value_r)) => value_l == value_r,
            (Self::Num(value_l), Self::Num(value_r)) => value_l == value_r,
            (Self::Text(value_l), Self::Text(value_r)) => value_l == value_r,
            (Self::Struct(fields_l), Self::Struct(fields_r)) => fields_l == fields_r,
            (Self::Case(case_l), Self::Case(case_r)) => eq_case(case_l, case_r),
            (Self::Tuple(values_l), Self::Tuple(values_r))
            | (Self::List(values_l), Self::List(values_r)) => values_l == values_r,
            (Self::Opt(value_l), Self::Opt(value_r)) => value_l == value_r,
            (Self::Func(id_l), Self::Func(id_r)) => id_l == id_r,
            (Self::Extern(value_l), Self::Extern(value_r)) => value_l == value_r,
            _ => false,
        }
    }
}

impl Eq for ValueKind {}

impl Hash for ValueKind {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        std::mem::discriminant(self).hash(hasher);
        match self {
            Self::Bool(value) => value.hash(hasher),
            Self::Num(value) => value.hash(hasher),
            Self::Text(value) => value.hash(hasher),
            Self::Struct(fields) => fields.hash(hasher),
            Self::Case(case) => hash_case(case, hasher),
            Self::Tuple(values) | Self::List(values) => values.hash(hasher),
            Self::Opt(value) => value.hash(hasher),
            Self::Func(id) => id.hash(hasher),
            Self::Extern(value) => value.hash(hasher),
        }
    }
}

// - Located case bodies

// Mixfix's general Eq/Hash ignore atom spans; exact value storage retains them
fn eq_case(case_l: &ValueCase, case_r: &ValueCase) -> bool {
    match (case_l, case_r) {
        (Mixfix::Arg(value_l), Mixfix::Arg(value_r)) => value_l == value_r,
        (Mixfix::Atom(atom_l), Mixfix::Atom(atom_r)) => atom_l == atom_r,
        (Mixfix::Brack(atom_l_l, case_l, atom_l_r), Mixfix::Brack(atom_r_l, case_r, atom_r_r)) => {
            atom_l_l == atom_r_l && eq_case(case_l, case_r) && atom_l_r == atom_r_r
        }
        (Mixfix::Infix(case_l_l, atom_l, case_l_r), Mixfix::Infix(case_r_l, atom_r, case_r_r)) => {
            eq_case(case_l_l, case_r_l) && atom_l == atom_r && eq_case(case_l_r, case_r_r)
        }
        (Mixfix::Seq(cases_l), Mixfix::Seq(cases_r)) => {
            cases_l.len() == cases_r.len()
                && cases_l
                    .iter()
                    .zip(cases_r)
                    .all(|(case_l, case_r)| eq_case(case_l, case_r))
        }
        _ => false,
    }
}

fn hash_case<H: Hasher>(case: &ValueCase, hasher: &mut H) {
    std::mem::discriminant(case).hash(hasher);
    match case {
        Mixfix::Arg(value) => value.hash(hasher),
        Mixfix::Atom(atom) => atom.hash(hasher),
        Mixfix::Brack(atom_l, case, atom_r) => {
            atom_l.hash(hasher);
            hash_case(case, hasher);
            atom_r.hash(hasher);
        }
        Mixfix::Infix(case_l, atom, case_r) => {
            hash_case(case_l, hasher);
            atom.hash(hasher);
            hash_case(case_r, hasher);
        }
        Mixfix::Seq(cases) => {
            cases.len().hash(hasher);
            for case in cases {
                hash_case(case, hasher);
            }
        }
    }
}

// = Canonical equality and hashing

impl ValueKind {
    pub(super) fn eq_canon(&self, kind_r: &Self, interner: &CanonInterner<Self>) -> bool {
        let eq_value = |value_l: &Value, value_r: &Value| {
            interner.canon_id(value_l.node) == interner.canon_id(value_r.node)
        };
        match (self, kind_r) {
            (ValueKind::Bool(value_l), ValueKind::Bool(value_r)) => value_l == value_r,
            (ValueKind::Num(value_l), ValueKind::Num(value_r)) => value_l == value_r,
            (ValueKind::Text(value_l), ValueKind::Text(value_r)) => value_l == value_r,
            (ValueKind::Struct(fields_l), ValueKind::Struct(fields_r)) => {
                fields_l.len() == fields_r.len()
                    && fields_l.iter().zip(fields_r).all(
                        |((atom_l, value_l), (atom_r, value_r))| {
                            atom_l.node == atom_r.node && eq_value(value_l, value_r)
                        },
                    )
            }
            (ValueKind::Case(case_l), ValueKind::Case(case_r)) => case_l.eq_by(case_r, eq_value),
            (ValueKind::Tuple(values_l), ValueKind::Tuple(values_r))
            | (ValueKind::List(values_l), ValueKind::List(values_r)) => {
                values_l.len() == values_r.len()
                    && values_l
                        .iter()
                        .zip(values_r)
                        .all(|(value_l, value_r)| eq_value(value_l, value_r))
            }
            (ValueKind::Opt(value_l), ValueKind::Opt(value_r)) => match (value_l, value_r) {
                (Some(value_l), Some(value_r)) => eq_value(value_l, value_r),
                (None, None) => true,
                _ => false,
            },
            (ValueKind::Func(id_l), ValueKind::Func(id_r)) => id_l.node == id_r.node,
            (ValueKind::Extern(value_l), ValueKind::Extern(value_r)) => value_l == value_r,
            _ => false,
        }
    }

    // - Hashing

    pub(super) fn hash_canon<H: Hasher>(&self, interner: &CanonInterner<Self>, hasher: &mut H) {
        std::mem::discriminant(self).hash(hasher);
        match self {
            ValueKind::Bool(value) => value.hash(hasher),
            ValueKind::Num(value) => value.hash(hasher),
            ValueKind::Text(value) => value.hash(hasher),
            ValueKind::Struct(fields) => {
                fields.len().hash(hasher);
                for (atom, value) in fields {
                    atom.node.hash(hasher);
                    interner.canon_id(value.node).hash(hasher);
                }
            }
            ValueKind::Case(case) => Self::hash_case_canon(case, interner, hasher),
            ValueKind::Tuple(values) | ValueKind::List(values) => {
                values.len().hash(hasher);
                for value in values {
                    interner.canon_id(value.node).hash(hasher);
                }
            }
            ValueKind::Opt(value) => value
                .map(|value| interner.canon_id(value.node))
                .hash(hasher),
            ValueKind::Func(id) => id.node.hash(hasher),
            ValueKind::Extern(value) => value.hash(hasher),
        }
    }

    fn hash_case_canon<H: Hasher>(
        case: &ValueCase,
        interner: &CanonInterner<Self>,
        hasher: &mut H,
    ) {
        std::mem::discriminant(case).hash(hasher);
        match case {
            Mixfix::Arg(value) => interner.canon_id(value.node).hash(hasher),
            Mixfix::Atom(atom) => atom.node.hash(hasher),
            Mixfix::Brack(atom_l, case, atom_r) => {
                atom_l.node.hash(hasher);
                Self::hash_case_canon(case, interner, hasher);
                atom_r.node.hash(hasher);
            }
            Mixfix::Infix(case_l, atom, case_r) => {
                Self::hash_case_canon(case_l, interner, hasher);
                atom.node.hash(hasher);
                Self::hash_case_canon(case_r, interner, hasher);
            }
            Mixfix::Seq(cases) => {
                cases.len().hash(hasher);
                for case in cases {
                    Self::hash_case_canon(case, interner, hasher);
                }
            }
        }
    }
}

// = Syntax comparison

impl SyntaxEq for ValueRef<'_> {
    fn syntax_eq(&self, value_other: &Self) -> bool {
        if std::ptr::eq(self.arena, value_other.arena) {
            self.arena.canon_id(&self.value) == value_other.arena.canon_id(&value_other.value)
        } else {
            self.syntax_cmp(value_other).is_eq()
        }
    }
}

impl SyntaxCmp for ValueRef<'_> {
    fn syntax_cmp(&self, value_other: &Self) -> Ordering {
        let compare_value = |value_l: &Value, value_r: &Value| {
            self.arena
                .view(*value_l)
                .syntax_cmp(&value_other.arena.view(*value_r))
        };
        let compare_values = |values_l: &[Value], values_r: &[Value]| {
            values_l
                .iter()
                .zip(values_r)
                .map(|(value_l, value_r)| compare_value(value_l, value_r))
                .find(|order| !order.is_eq())
                .unwrap_or_else(|| values_l.len().cmp(&values_r.len()))
        };
        let kind_l = self.arena.kind(&self.value);
        let kind_r = value_other.arena.kind(&value_other.value);
        match (kind_l, kind_r) {
            (ValueKind::Bool(value_l), ValueKind::Bool(value_r)) => value_l.cmp(value_r),
            (ValueKind::Num(value_l), ValueKind::Num(value_r)) => num::compare(value_l, value_r),
            (ValueKind::Text(value_l), ValueKind::Text(value_r)) => value_l.cmp(value_r),
            (ValueKind::Struct(fields_l), ValueKind::Struct(fields_r)) => fields_l
                .iter()
                .zip(fields_r)
                .map(|((atom_l, value_l), (atom_r, value_r))| {
                    atom_l
                        .node
                        .cmp(&atom_r.node)
                        .then_with(|| compare_value(value_l, value_r))
                })
                .find(|order| !order.is_eq())
                .unwrap_or_else(|| fields_l.len().cmp(&fields_r.len())),
            (ValueKind::Case(case_l), ValueKind::Case(case_r)) => {
                case_l.cmp_by(case_r, compare_value)
            }
            (ValueKind::Tuple(values_l), ValueKind::Tuple(values_r))
            | (ValueKind::List(values_l), ValueKind::List(values_r)) => {
                compare_values(values_l, values_r)
            }
            (ValueKind::Opt(value_l), ValueKind::Opt(value_r)) => match (value_l, value_r) {
                (Some(value_l), Some(value_r)) => compare_value(value_l, value_r),
                (None, Some(_)) => Ordering::Less,
                (Some(_), None) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            },
            (ValueKind::Func(id_l), ValueKind::Func(id_r)) => id_l.node.cmp(&id_r.node),
            (ValueKind::Extern(value_l), ValueKind::Extern(value_r)) => value_l.cmp(value_r),
            _ => kind_l.tag().cmp(&kind_r.tag()),
        }
    }
}

// = Errors

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ValueError {
    #[error("value arena index overflow")]
    IndexOverflow,
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

// - Index overflow

impl From<TryFromIntError> for ValueError {
    fn from(_: TryFromIntError) -> Self {
        Self::IndexOverflow
    }
}
