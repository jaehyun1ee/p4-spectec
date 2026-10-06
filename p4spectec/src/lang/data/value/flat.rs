//! Flat values with interned bodies, types, and spans
//!
//! Child values refer to the same arena; cases hold a mixop handle.
//! Exact equality includes stored annotations, while canonical identity
//! compares value contents independently of their source locations.

use super::external::{DecodeContext, EncodeContext};
use crate::lang::{
    common::{
        Id,
        notation::atom::Atom,
        prim::num::Number,
        source::{NotePhrase, Phrase, Span},
    },
    data::{
        intern::{CanonEq, CanonHash, CanonInterner, Interned},
        notation::{Mixfix, MixopArena, MixopId},
        typ::TypKind,
    },
};
use crate::util::json::json;
use serde_derive_state::{DeserializeState, SerializeState};
use std::{
    hash::{Hash, Hasher},
    rc::Rc,
};

/// A value's body, type, and span handles in one arena.
pub type ValueFlat = NotePhrase<Interned<ValueFlatKind>, Interned<TypKind>, Interned<Span>>;
/// A named value field.
pub type ValueField = (Phrase<Atom>, ValueFlat);
/// A case with one value per argument position.
pub type ValueCase = Mixfix<MixopId, ValueFlat>;

/// A stored value body whose children belong to the same arena.
#[derive(Debug, PartialEq, Eq, Hash, SerializeState, DeserializeState)]
#[serde(rename = "ValueKind")]
#[serde(serialize_state = "EncodeContext<'arena>", ser_parameters = "'arena")]
#[serde(deserialize_state = "DecodeContext<'de>")]
pub enum ValueFlatKind {
    Bool(bool),
    Num(Number),
    Text(String),
    Struct(#[serde(state)] Vec<ValueField>),
    Case(#[serde(state)] ValueCase),
    Tuple(#[serde(state)] Vec<ValueFlat>),
    Opt(#[serde(state)] Option<ValueFlat>),
    List(#[serde(state)] Vec<ValueFlat>),
    Func(#[serde(state)] Id),
    Extern(Rc<json>),
}

// - Tags

/// The kind of a value without its payload, for errors and ordering.
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

impl ValueFlatKind {
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

impl CanonEq<MixopArena> for ValueFlatKind {
    fn canon_eq(
        &self,
        interner: &CanonInterner<Self>,
        arena_mixop: &MixopArena,
        kind_r: &Self,
    ) -> bool {
        // Children compare by canonical id, computed when they were interned
        let eq_value = |value_l: &ValueFlat, value_r: &ValueFlat| {
            interner.canon_id(value_l.node) == interner.canon_id(value_r.node)
        };
        match (self, kind_r) {
            (ValueFlatKind::Bool(value_l), ValueFlatKind::Bool(value_r)) => value_l == value_r,
            (ValueFlatKind::Num(value_l), ValueFlatKind::Num(value_r)) => value_l == value_r,
            (ValueFlatKind::Text(value_l), ValueFlatKind::Text(value_r)) => value_l == value_r,
            (ValueFlatKind::Struct(value_fields_l), ValueFlatKind::Struct(value_fields_r)) => {
                value_fields_l.len() == value_fields_r.len()
                    && value_fields_l.iter().zip(value_fields_r).all(
                        |((atom_l, value_l), (atom_r, value_r))| {
                            atom_l.node == atom_r.node && eq_value(value_l, value_r)
                        },
                    )
            }
            (ValueFlatKind::Case(value_case_l), ValueFlatKind::Case(value_case_r)) => {
                arena_mixop.canon_eq(*value_case_l.mixop(), *value_case_r.mixop())
                    && value_case_l.args().len() == value_case_r.args().len()
                    && value_case_l
                        .args()
                        .iter()
                        .zip(value_case_r.args())
                        .all(|(value_l, value_r)| eq_value(value_l, value_r))
            }
            (ValueFlatKind::Tuple(values_l), ValueFlatKind::Tuple(values_r))
            | (ValueFlatKind::List(values_l), ValueFlatKind::List(values_r)) => {
                values_l.len() == values_r.len()
                    && values_l
                        .iter()
                        .zip(values_r)
                        .all(|(value_l, value_r)| eq_value(value_l, value_r))
            }
            (ValueFlatKind::Opt(value_l), ValueFlatKind::Opt(value_r)) => {
                match (value_l, value_r) {
                    (Some(value_l), Some(value_r)) => eq_value(value_l, value_r),
                    (None, None) => true,
                    _ => false,
                }
            }
            (ValueFlatKind::Func(id_l), ValueFlatKind::Func(id_r)) => id_l.node == id_r.node,
            (ValueFlatKind::Extern(json_l), ValueFlatKind::Extern(json_r)) => json_l == json_r,
            _ => false,
        }
    }
}

impl CanonHash<MixopArena> for ValueFlatKind {
    fn canon_hash<H: Hasher>(
        &self,
        interner: &CanonInterner<Self>,
        arena_mixop: &MixopArena,
        hasher: &mut H,
    ) {
        std::mem::discriminant(self).hash(hasher);
        match self {
            ValueFlatKind::Bool(value) => value.hash(hasher),
            ValueFlatKind::Num(value) => value.hash(hasher),
            ValueFlatKind::Text(value) => value.hash(hasher),
            ValueFlatKind::Struct(value_fields) => {
                value_fields.len().hash(hasher);
                for (atom, value) in value_fields {
                    atom.node.hash(hasher);
                    interner.canon_id(value.node).hash(hasher);
                }
            }
            ValueFlatKind::Case(value_case) => {
                arena_mixop.canon_id(*value_case.mixop()).hash(hasher);
                value_case.args().len().hash(hasher);
                for value in value_case.args() {
                    interner.canon_id(value.node).hash(hasher);
                }
            }
            ValueFlatKind::Tuple(values) | ValueFlatKind::List(values) => {
                values.len().hash(hasher);
                for value in values {
                    interner.canon_id(value.node).hash(hasher);
                }
            }
            ValueFlatKind::Opt(value) => value
                .map(|value| interner.canon_id(value.node))
                .hash(hasher),
            ValueFlatKind::Func(id) => id.node.hash(hasher),
            ValueFlatKind::Extern(json) => json.hash(hasher),
        }
    }
}
