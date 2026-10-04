//! Borrowed primitive payloads for exact value interning
//!
//! Literal evaluation avoids cloning payloads already held by the arena.
//! Queries hash and compare exactly as their owned value bodies.

use super::ValueKind;
use crate::lang::common::prim::num::Number;
use hashbrown::Equivalent;
use std::hash::{Hash, Hasher};

/// Borrows a primitive until the interner needs to store it.
pub(super) enum Primitive<'a> {
    Num(&'a Number),
    Text(&'a str),
}

impl Primitive<'_> {
    /// Clones the primitive only when no equal body has been stored.
    pub(super) fn into_kind(self) -> ValueKind {
        match self {
            Self::Num(num) => ValueKind::Num(num.clone()),
            Self::Text(text) => ValueKind::Text(text.to_owned()),
        }
    }
}

impl Hash for Primitive<'_> {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        match self {
            Self::Num(num) => {
                std::mem::discriminant(&ValueKind::Num(Number::Int(num_bigint::BigInt::ZERO)))
                    .hash(hasher);
                num.hash(hasher);
            }
            Self::Text(text) => {
                std::mem::discriminant(&ValueKind::Text(String::new())).hash(hasher);
                text.hash(hasher);
            }
        }
    }
}

impl Equivalent<ValueKind> for Primitive<'_> {
    fn equivalent(&self, kind: &ValueKind) -> bool {
        match (self, kind) {
            (Self::Num(num), ValueKind::Num(num_stored)) => *num == num_stored,
            (Self::Text(text), ValueKind::Text(text_stored)) => *text == text_stored,
            _ => false,
        }
    }
}
