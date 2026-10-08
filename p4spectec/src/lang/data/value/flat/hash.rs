//! Hashing flat values
//!
//! Hashes contents in variant order, using canonical child identities where stored.

use std::hash::{Hash, Hasher};

use crate::lang::data::{
    intern::{CanonHash, CanonInterner},
    notation::{MixopArena, flat::get as get_notation},
};

use super::ValueKind;

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
                arena_mixop
                    .canon_id(*get_notation::mixop(value_case))
                    .hash(hasher);
                get_notation::args(value_case).len().hash(hasher);
                for value in get_notation::args(value_case) {
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
