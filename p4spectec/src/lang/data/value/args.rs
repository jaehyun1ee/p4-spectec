//! Temporary composite arguments, materialized only for new exact bodies
//!
//! Short argument lists stay inline during evaluation.
//! `ValueParts` compares every child handle against stored bodies;
//! the common interner converts it to a `ValueKind` only on an exact miss.

use std::hash::{Hash, Hasher};

use hashbrown::Equivalent;
use smallvec::SmallVec;

use crate::lang::data::notation::{Shape, ShapeArena};

use super::{Value, ValueCase, ValueKind, ValueTag, handle::hash_parts};

/// Temporary arguments stored inline until they exceed four values.
pub type ValueArgs = SmallVec<[Value; 4]>;

/// An exact composite-body query with owned or borrowed arguments.
pub(super) enum ValueParts<Values> {
    Case(Shape, Values),
    Tuple(Values),
    List(Values),
}

impl<Values> ValueParts<Values> {
    /// Moves the arguments into the existing owned representation.
    pub(super) fn into_kind(
        self,
        arena_shape: &ShapeArena,
        into_values: impl FnOnce(Values) -> Vec<Value>,
    ) -> ValueKind {
        match self {
            Self::Case(shape, values) => ValueKind::Case(
                ValueCase::new_in(arena_shape, shape, into_values(values))
                    .expect("a mixfix fills every position"),
            ),
            Self::Tuple(values) => ValueKind::Tuple(into_values(values)),
            Self::List(values) => ValueKind::List(into_values(values)),
        }
    }
}

impl<Values: AsRef<[Value]>> Hash for ValueParts<Values> {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        match self {
            Self::Case(shape, values) => {
                hash_parts(ValueTag::Case, Some(*shape), values.as_ref(), hasher)
            }
            Self::Tuple(values) => hash_parts(ValueTag::Tuple, None, values.as_ref(), hasher),
            Self::List(values) => hash_parts(ValueTag::List, None, values.as_ref(), hasher),
        }
    }
}

impl<Values: AsRef<[Value]>> Equivalent<ValueKind> for ValueParts<Values> {
    fn equivalent(&self, kind: &ValueKind) -> bool {
        match (self, kind) {
            (Self::Case(shape, values), ValueKind::Case(value_case)) => {
                shape == value_case.mixop() && values.as_ref() == value_case.args()
            }
            (Self::Tuple(values), ValueKind::Tuple(values_stored))
            | (Self::List(values), ValueKind::List(values_stored)) => {
                values.as_ref() == values_stored.as_slice()
            }
            _ => false,
        }
    }
}
