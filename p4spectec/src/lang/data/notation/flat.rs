//! Flat notation handles and their stored bodies
//!
//! `Mixop` is a handle to a `MixopKind`, whose children are handles
//! into a `MixopArena` rather than nested inside it:
//! `Brack(atom_l, mixop, atom_r)` refers to its inner mixop by handle,
//! and equal subtrees are stored once.
//! The forms themselves are unchanged; a sequence keeps its elements.
//! Exact equality and hashing include atom spans and compare children
//! by handle; `CanonEq` and `CanonHash` read atom names
//! and children's canonical ids, so they ignore spans.

use super::external::{DecodeContext, EncodeContext};
use serde_derive_state::{DeserializeState, SerializeState};

use std::hash::{Hash, Hasher};

use crate::lang::data::intern::{CanonEq, CanonHash, CanonInterner, Interned};

use super::AtomPhrase;

/// A notation handle valid only in its issuing arena.
pub type Mixop = Interned<MixopKind>;

/// One interned notation node, with child handles in the same arena.
#[derive(Debug, PartialEq, Eq, Hash, SerializeState, DeserializeState)]
#[serde(rename = "Mixop")]
#[serde(serialize_state = "EncodeContext<'arena>", ser_parameters = "'arena")]
#[serde(deserialize_state = "DecodeContext<'de>")]
pub enum MixopKind {
    Arg,
    Atom(#[serde(state)] AtomPhrase),
    Brack(#[serde(state)] AtomPhrase, #[serde(state)] Mixop, #[serde(state)] AtomPhrase),
    Infix(#[serde(state)] Mixop, #[serde(state)] AtomPhrase, #[serde(state)] Mixop),
    Seq(#[serde(state)] Vec<Mixop>),
}

impl MixopKind {
    /// Orders the variants for comparison across forms.
    pub(crate) fn tag(&self) -> u8 {
        match self {
            Self::Arg => 0,
            Self::Atom(_) => 1,
            Self::Brack(..) => 2,
            Self::Infix(..) => 3,
            Self::Seq(_) => 4,
        }
    }
}

// = Canonical equality and hashing

// Canonical identity: atom names and children's canonical ids, so spans
// are ignored as a tree ignores them

impl CanonEq for MixopKind {
    fn canon_eq(&self, interner: &CanonInterner<Self>, _: &(), kind_r: &Self) -> bool {
        // Children compare by canonical id, computed when they were interned
        let eq_mixop = |mixop_id_l: &Mixop, mixop_id_r: &Mixop| {
            interner.canon_id(*mixop_id_l) == interner.canon_id(*mixop_id_r)
        };
        match (self, kind_r) {
            (Self::Arg, Self::Arg) => true,
            (Self::Atom(atom_l), Self::Atom(atom_r)) => atom_l.node == atom_r.node,
            (
                Self::Brack(atom_l_l, mixop_id_l, atom_l_r),
                Self::Brack(atom_r_l, mixop_id_r, atom_r_r),
            ) => {
                atom_l_l.node == atom_r_l.node
                    && eq_mixop(mixop_id_l, mixop_id_r)
                    && atom_l_r.node == atom_r_r.node
            }
            (
                Self::Infix(mixop_id_l_l, atom_l, mixop_id_l_r),
                Self::Infix(mixop_id_r_l, atom_r, mixop_id_r_r),
            ) => {
                eq_mixop(mixop_id_l_l, mixop_id_r_l)
                    && atom_l.node == atom_r.node
                    && eq_mixop(mixop_id_l_r, mixop_id_r_r)
            }
            (Self::Seq(mixops_l), Self::Seq(mixops_r)) => {
                mixops_l.len() == mixops_r.len()
                    && mixops_l
                        .iter()
                        .zip(mixops_r)
                        .all(|(mixop_id_l, mixop_id_r)| eq_mixop(mixop_id_l, mixop_id_r))
            }
            _ => false,
        }
    }
}

impl CanonHash for MixopKind {
    fn canon_hash<H: Hasher>(&self, interner: &CanonInterner<Self>, _: &(), hasher: &mut H) {
        self.tag().hash(hasher);
        match self {
            Self::Arg => {}
            Self::Atom(atom) => atom.node.hash(hasher),
            Self::Brack(atom_l, mixop_id, atom_r) => {
                atom_l.node.hash(hasher);
                interner.canon_id(*mixop_id).hash(hasher);
                atom_r.node.hash(hasher);
            }
            Self::Infix(mixop_id_l, atom, mixop_id_r) => {
                interner.canon_id(*mixop_id_l).hash(hasher);
                atom.node.hash(hasher);
                interner.canon_id(*mixop_id_r).hash(hasher);
            }
            Self::Seq(mixops) => {
                mixops.len().hash(hasher);
                for mixop_id in mixops {
                    interner.canon_id(*mixop_id).hash(hasher);
                }
            }
        }
    }
}
