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

use std::{
    cmp::Ordering,
    hash::{Hash, Hasher},
};

use serde_derive_state::{DeserializeState, SerializeState};

use crate::lang::data::intern::{CanonEq, CanonHash, CanonInterner, Interned};

use super::{
    AtomPhrase, MixopArena, Piece,
    external::{DecodeContext, EncodeContext},
    tree,
};

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
    fn tag(&self) -> u8 {
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

// = Comparison and traversal

/// Orders two mixops by structure and atom names, lexicographically.
///
/// Brackets compare the opening atom, the inner form, then the closing atom;
/// infix compares the left form, the operator, then the right form;
/// sequences compare the common prefix, then the length;
/// different forms order by variant.
/// Both walks visit equal prefixes, so they reach argument positions in step,
/// and `compare_arg` orders the arguments at each position.
pub(super) fn cmp_by(
    arena_mixop_l: &MixopArena,
    mixop_l: Mixop,
    arena_mixop_r: &MixopArena,
    mixop_r: Mixop,
    mut compare_arg: impl FnMut(usize) -> Ordering,
) -> Ordering {
    let mut pos = 0;
    arena_mixop_l.kind(mixop_l).cmp_by_inner(
        arena_mixop_l,
        arena_mixop_r.kind(mixop_r),
        arena_mixop_r,
        &mut pos,
        &mut compare_arg,
    )
}

impl MixopKind {
    /// Structural comparison, threading the position and the argument comparator.
    fn cmp_by_inner(
        &self,
        arena_mixop_l: &MixopArena,
        kind_r: &Self,
        arena_mixop_r: &MixopArena,
        pos: &mut usize,
        compare_arg: &mut impl FnMut(usize) -> Ordering,
    ) -> Ordering {
        match (self, kind_r) {
            // Arguments by the caller's comparator
            (MixopKind::Arg, MixopKind::Arg) => {
                let order = compare_arg(*pos);
                *pos += 1;
                order
            }
            // Atoms by name
            (MixopKind::Atom(atom_l), MixopKind::Atom(atom_r)) => atom_l.node.cmp(&atom_r.node),
            // Brackets: opening atom, inner form, closing atom
            (
                MixopKind::Brack(atom_l_l, child_l, atom_l_r),
                MixopKind::Brack(atom_r_l, child_r, atom_r_r),
            ) => atom_l_l
                .node
                .cmp(&atom_r_l.node)
                .then_with(|| {
                    arena_mixop_l.kind(*child_l).cmp_by_inner(
                        arena_mixop_l,
                        arena_mixop_r.kind(*child_r),
                        arena_mixop_r,
                        pos,
                        compare_arg,
                    )
                })
                .then_with(|| atom_l_r.node.cmp(&atom_r_r.node)),
            // Infix: left form, operator, right form
            (
                MixopKind::Infix(child_l_l, atom_l, child_l_r),
                MixopKind::Infix(child_r_l, atom_r, child_r_r),
            ) => arena_mixop_l
                .kind(*child_l_l)
                .cmp_by_inner(
                    arena_mixop_l,
                    arena_mixop_r.kind(*child_r_l),
                    arena_mixop_r,
                    pos,
                    compare_arg,
                )
                .then_with(|| atom_l.node.cmp(&atom_r.node))
                .then_with(|| {
                    arena_mixop_l.kind(*child_l_r).cmp_by_inner(
                        arena_mixop_l,
                        arena_mixop_r.kind(*child_r_r),
                        arena_mixop_r,
                        pos,
                        compare_arg,
                    )
                }),
            // Sequences: common prefix first, then length
            (MixopKind::Seq(elems_l), MixopKind::Seq(elems_r)) => {
                let kinds_l = elems_l.iter().map(|elem| arena_mixop_l.kind(*elem));
                let kinds_r = elems_r.iter().map(|elem| arena_mixop_r.kind(*elem));
                for (kind_l, kind_r) in kinds_l.zip(kinds_r) {
                    let order =
                        kind_l.cmp_by_inner(arena_mixop_l, kind_r, arena_mixop_r, pos, compare_arg);
                    if order != Ordering::Equal {
                        return order;
                    }
                }
                elems_l.len().cmp(&elems_r.len())
            }
            // Different forms order by variant
            _ => self.tag().cmp(&kind_r.tag()),
        }
    }

    /// Visits a stored node, threading the argument position.
    fn visit_inner<'a>(
        &'a self,
        arena_mixop: &'a MixopArena,
        pos: &mut usize,
        visit_piece: &mut impl FnMut(Piece<'a>),
    ) {
        match self {
            MixopKind::Arg => {
                visit_piece(Piece::Arg(*pos));
                *pos += 1;
            }
            MixopKind::Atom(atom) => visit_piece(Piece::Atom(atom)),
            MixopKind::Brack(atom_l, child, atom_r) => {
                visit_piece(Piece::Atom(atom_l));
                arena_mixop
                    .kind(*child)
                    .visit_inner(arena_mixop, pos, visit_piece);
                visit_piece(Piece::Atom(atom_r));
            }
            MixopKind::Infix(child_l, atom, child_r) => {
                arena_mixop
                    .kind(*child_l)
                    .visit_inner(arena_mixop, pos, visit_piece);
                visit_piece(Piece::Atom(atom));
                arena_mixop
                    .kind(*child_r)
                    .visit_inner(arena_mixop, pos, visit_piece);
            }
            MixopKind::Seq(elems) => {
                for elem in elems {
                    arena_mixop
                        .kind(*elem)
                        .visit_inner(arena_mixop, pos, visit_piece);
                }
            }
        }
    }
}

/// Visits atoms and argument positions in reading order.
pub(crate) fn visit<'a>(
    arena_mixop: &'a MixopArena,
    mixop: Mixop,
    mut visit_piece: impl FnMut(Piece<'a>),
) {
    let mut pos = 0;
    arena_mixop
        .kind(mixop)
        .visit_inner(arena_mixop, &mut pos, &mut visit_piece);
}

/// Compares a stored form with an owned tree, ignoring atom spans.
pub(crate) fn matches_tree(
    arena_mixop: &MixopArena,
    mixop_flat: Mixop,
    mixop_tree: &tree::Mixop,
) -> bool {
    match (arena_mixop.kind(mixop_flat), mixop_tree) {
        (MixopKind::Arg, tree::Mixop::Arg) => true,
        (MixopKind::Atom(atom_l), tree::Mixop::Atom(atom_r)) => atom_l.node == atom_r.node,
        (
            MixopKind::Brack(atom_l_l, child_l, atom_l_r),
            tree::Mixop::Brack(atom_r_l, child_r, atom_r_r),
        ) => {
            atom_l_l.node == atom_r_l.node
                && atom_l_r.node == atom_r_r.node
                && matches_tree(arena_mixop, *child_l, child_r)
        }
        (
            MixopKind::Infix(child_l_l, atom_l, child_l_r),
            tree::Mixop::Infix(child_r_l, atom_r, child_r_r),
        ) => {
            atom_l.node == atom_r.node
                && matches_tree(arena_mixop, *child_l_l, child_r_l)
                && matches_tree(arena_mixop, *child_l_r, child_r_r)
        }
        (MixopKind::Seq(children_l), tree::Mixop::Seq(children_r)) => {
            children_l.len() == children_r.len()
                && children_l
                    .iter()
                    .zip(children_r)
                    .all(|(child_l, child_r)| matches_tree(arena_mixop, *child_l, child_r))
        }
        _ => false,
    }
}
