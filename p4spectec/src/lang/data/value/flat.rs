//! Flat values with interned bodies, types, and spans
//!
//! Child values refer to the same arena; cases hold a mixop handle.
//! Exact equality includes stored annotations, while canonical identity
//! compares value contents independently of their source locations.

use std::{
    hash::{Hash, Hasher},
    rc::Rc,
};

use serde_derive_state::{DeserializeState, SerializeState};

use crate::util::json::json;

use crate::lang::{
    common::{
        Id,
        notation::atom::Atom,
        prim::num::Number,
        source::{NotePhrase, Phrase, Span},
    },
    data::{
        intern::{CanonEq, CanonHash, CanonInterner, Interned},
        notation::{Mixfix, MixopArena, flat::Mixop},
        typ::TypKind,
    },
};

use super::{
    ValueTag,
    external::{DecodeContext, EncodeContext},
};

pub use super::view::ValueRef;

/// A value's body, type, and span handles in one arena.
pub type Value = NotePhrase<Interned<ValueKind>, Interned<TypKind>, Interned<Span>>;

/// A named value field.
pub type ValueField = (Phrase<Atom>, Value);
/// A case with one value per argument position.
pub type ValueCase = Mixfix<Mixop, Value>;

/// A stored value body whose children belong to the same arena.
#[derive(Debug, PartialEq, Eq, Hash, SerializeState, DeserializeState)]
#[serde(rename = "ValueKind")]
#[serde(serialize_state = "EncodeContext<'arena>", ser_parameters = "'arena")]
#[serde(deserialize_state = "DecodeContext<'de>")]
pub enum ValueKind {
    Bool(bool),
    Num(Number),
    Text(String),
    Struct(#[serde(state)] Vec<ValueField>),
    Case(#[serde(state)] ValueCase),
    Tuple(#[serde(state)] Vec<Value>),
    Opt(#[serde(state)] Option<Value>),
    List(#[serde(state)] Vec<Value>),
    Func(#[serde(state)] Id),
    Extern(Rc<json>),
}

// - Tags

impl ValueKind {
    /// The kind of this body.
    pub(crate) fn tag(&self) -> ValueTag {
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
// = Canonical equality and hashing

impl CanonEq<MixopArena> for ValueKind {
    fn canon_eq(
        &self,
        interner: &CanonInterner<Self>,
        arena_mixop: &MixopArena,
        kind_r: &Self,
    ) -> bool {
        // Children compare by canonical id, computed when they were interned
        let eq_value = |value_l: &Value, value_r: &Value| {
            interner.canon_id(value_l.node) == interner.canon_id(value_r.node)
        };
        match (self, kind_r) {
            (ValueKind::Bool(value_l), ValueKind::Bool(value_r)) => value_l == value_r,
            (ValueKind::Num(value_l), ValueKind::Num(value_r)) => value_l == value_r,
            (ValueKind::Text(value_l), ValueKind::Text(value_r)) => value_l == value_r,
            (ValueKind::Struct(value_fields_l), ValueKind::Struct(value_fields_r)) => {
                value_fields_l.len() == value_fields_r.len()
                    && value_fields_l.iter().zip(value_fields_r).all(
                        |((atom_l, value_l), (atom_r, value_r))| {
                            atom_l.node == atom_r.node && eq_value(value_l, value_r)
                        },
                    )
            }
            (ValueKind::Case(value_case_l), ValueKind::Case(value_case_r)) => {
                arena_mixop.canon_eq(*value_case_l.mixop(), *value_case_r.mixop())
                    && value_case_l.args().len() == value_case_r.args().len()
                    && value_case_l
                        .args()
                        .iter()
                        .zip(value_case_r.args())
                        .all(|(value_l, value_r)| eq_value(value_l, value_r))
            }
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
            (ValueKind::Extern(json_l), ValueKind::Extern(json_r)) => json_l == json_r,
            _ => false,
        }
    }
}

impl CanonHash<MixopArena> for ValueKind {
    fn canon_hash<H: Hasher>(
        &self,
        interner: &CanonInterner<Self>,
        arena_mixop: &MixopArena,
        hasher: &mut H,
    ) {
        std::mem::discriminant(self).hash(hasher);
        match self {
            ValueKind::Bool(value) => value.hash(hasher),
            ValueKind::Num(value) => value.hash(hasher),
            ValueKind::Text(value) => value.hash(hasher),
            ValueKind::Struct(value_fields) => {
                value_fields.len().hash(hasher);
                for (atom, value) in value_fields {
                    atom.node.hash(hasher);
                    interner.canon_id(value.node).hash(hasher);
                }
            }
            ValueKind::Case(value_case) => {
                arena_mixop.canon_id(*value_case.mixop()).hash(hasher);
                value_case.args().len().hash(hasher);
                for value in value_case.args() {
                    interner.canon_id(value.node).hash(hasher);
                }
            }
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
            ValueKind::Extern(json) => json.hash(hasher),
        }
    }
}
