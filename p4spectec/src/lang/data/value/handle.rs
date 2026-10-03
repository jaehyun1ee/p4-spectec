//! Handle representation of values: arena bodies and case shapes
//!
//! A `Value` is three interned handles: body, type, and span.
//! `Handle` holds a body's children as handles into the same `Arena`
//! and a case as its notation shape with argument handles.
//! Bodies are stored exactly, spans included, so two values that print alike
//! may still be distinct entries;
//! the canonical identity ignores spans and is what syntax equality uses.

use std::{
    fmt,
    hash::{Hash, Hasher},
};

use crate::lang::{
    common::{
        notation::atom::Atom,
        source::{NotePhrase, Phrase, Span},
    },
    data::{
        intern::{CanonEq, CanonHash, CanonInterner, Interned},
        notation::{Mixfix, Shape, ShapeArena},
        typ::TypKind,
    },
};

use super::node::{ValueNode, ValueRepr};

// = Representation

/// Children held as handles into the same `Arena`.
#[derive(Clone, Copy, Debug)]
pub struct Handle;

impl ValueRepr for Handle {
    type Child = Value;
    type Elem = Value;
    type Mixop = Shape;
}

// = Values

/// A value handle: interned body, type, and span, valid in one arena.
pub type Value = NotePhrase<Interned<ValueKind>, Interned<TypKind>, Interned<Span>>;
/// A struct field: atom and value.
pub type ValueField = (Phrase<Atom>, Value);
/// A value body; children are handles into the same arena.
pub type ValueKind = ValueNode<Handle>;
/// A variant case: a notation shape and one value per position.
///
/// The shape belongs to the `ShapeArena` of the arena holding the case.
pub type ValueCase = Mixfix<Shape, Value>;

// = Exact body equality and hashing

// Exact identity: every field as stored, a case by its shape handle,
// which the shape arena interns with atom spans

impl PartialEq for ValueKind {
    fn eq(&self, kind_other: &Self) -> bool {
        match (self, kind_other) {
            (Self::Bool(value_l), Self::Bool(value_r)) => value_l == value_r,
            (Self::Num(value_l), Self::Num(value_r)) => value_l == value_r,
            (Self::Text(value_l), Self::Text(value_r)) => value_l == value_r,
            (Self::Struct(value_fields_l), Self::Struct(value_fields_r)) => {
                value_fields_l == value_fields_r
            }
            (Self::Case(value_case_l), Self::Case(value_case_r)) => value_case_l == value_case_r,
            (Self::Tuple(values_l), Self::Tuple(values_r))
            | (Self::List(values_l), Self::List(values_r)) => values_l == values_r,
            (Self::Opt(value_l), Self::Opt(value_r)) => value_l == value_r,
            (Self::Func(id_l), Self::Func(id_r)) => id_l == id_r,
            (Self::Extern(json_l), Self::Extern(json_r)) => json_l == json_r,
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
            Self::Struct(value_fields) => value_fields.hash(hasher),
            Self::Case(value_case) => value_case.hash(hasher),
            Self::Tuple(values) | Self::List(values) => values.hash(hasher),
            Self::Opt(value) => value.hash(hasher),
            Self::Func(id) => id.hash(hasher),
            Self::Extern(json) => json.hash(hasher),
        }
    }
}

// - Debugging

// The same text a derive prints: variant names and fields, no type name
impl fmt::Debug for ValueKind {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(value) => fmt.debug_tuple("Bool").field(value).finish(),
            Self::Num(num) => fmt.debug_tuple("Num").field(num).finish(),
            Self::Text(text) => fmt.debug_tuple("Text").field(text).finish(),
            Self::Struct(value_fields) => fmt.debug_tuple("Struct").field(value_fields).finish(),
            Self::Case(value_case) => fmt.debug_tuple("Case").field(value_case).finish(),
            Self::Tuple(values) => fmt.debug_tuple("Tuple").field(values).finish(),
            Self::Opt(value) => fmt.debug_tuple("Opt").field(value).finish(),
            Self::List(values) => fmt.debug_tuple("List").field(values).finish(),
            Self::Func(id) => fmt.debug_tuple("Func").field(id).finish(),
            Self::Extern(json) => fmt.debug_tuple("Extern").field(json).finish(),
        }
    }
}

// = Canonical equality and hashing

impl CanonEq<ShapeArena> for ValueKind {
    fn canon_eq(
        &self,
        interner: &CanonInterner<Self>,
        arena_shape: &ShapeArena,
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
                arena_shape.canon_eq(*value_case_l.mixop(), *value_case_r.mixop())
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

impl CanonHash<ShapeArena> for ValueKind {
    fn canon_hash<H: Hasher>(
        &self,
        interner: &CanonInterner<Self>,
        arena_shape: &ShapeArena,
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
                arena_shape.canon_id(*value_case.mixop()).hash(hasher);
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
