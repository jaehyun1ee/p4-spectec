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

// = Notation forms

/// A notation handle valid only in its issuing arena.
pub type Mixop = Interned<MixopKind>;

/// An interned notation with one argument per position.
pub type Mixfix<T> = super::Mixfix<Mixop, T>;

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

// = Structural properties

impl MixopKind {
    /// Counts positions from the children's recorded counts.
    pub(super) fn arity(&self, arena_mixop: &MixopArena) -> usize {
        match self {
            Self::Arg => 1,
            Self::Atom(_) => 0,
            Self::Brack(_, mixop, _) => arena_mixop.arity(*mixop),
            Self::Infix(mixop_l, _, mixop_r) => {
                arena_mixop.arity(*mixop_l) + arena_mixop.arity(*mixop_r)
            }
            Self::Seq(mixops) => mixops.iter().map(|mixop| arena_mixop.arity(*mixop)).sum(),
        }
    }
}

// = Canonical equality and hashing

impl CanonEq for MixopKind {
    fn canon_eq(&self, interner: &CanonInterner<Self>, _: &(), kind_r: &Self) -> bool {
        // Children compare by canonical id, computed when they were interned
        let eq_mixop = |mixop_l: &Mixop, mixop_r: &Mixop| {
            interner.canon_id(*mixop_l) == interner.canon_id(*mixop_r)
        };
        match (self, kind_r) {
            (Self::Arg, Self::Arg) => true,
            (Self::Atom(atom_l), Self::Atom(atom_r)) => atom_l.node == atom_r.node,
            (
                Self::Brack(atom_l_l, mixop_l, atom_l_r),
                Self::Brack(atom_r_l, mixop_r, atom_r_r),
            ) => {
                atom_l_l.node == atom_r_l.node
                    && eq_mixop(mixop_l, mixop_r)
                    && atom_l_r.node == atom_r_r.node
            }
            (
                Self::Infix(mixop_l_l, atom_l, mixop_l_r),
                Self::Infix(mixop_r_l, atom_r, mixop_r_r),
            ) => {
                eq_mixop(mixop_l_l, mixop_r_l)
                    && atom_l.node == atom_r.node
                    && eq_mixop(mixop_l_r, mixop_r_r)
            }
            (Self::Seq(mixops_l), Self::Seq(mixops_r)) => {
                mixops_l.len() == mixops_r.len()
                    && mixops_l
                        .iter()
                        .zip(mixops_r)
                        .all(|(mixop_l, mixop_r)| eq_mixop(mixop_l, mixop_r))
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
            Self::Brack(atom_l, mixop, atom_r) => {
                atom_l.node.hash(hasher);
                interner.canon_id(*mixop).hash(hasher);
                atom_r.node.hash(hasher);
            }
            Self::Infix(mixop_l, atom, mixop_r) => {
                interner.canon_id(*mixop_l).hash(hasher);
                atom.node.hash(hasher);
                interner.canon_id(*mixop_r).hash(hasher);
            }
            Self::Seq(mixops) => {
                mixops.len().hash(hasher);
                for mixop in mixops {
                    interner.canon_id(*mixop).hash(hasher);
                }
            }
        }
    }
}

// = Structural comparison

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

/// Compares a stored form with an owned tree, ignoring atom spans.
pub(crate) fn matches_tree(
    arena_mixop: &MixopArena,
    mixop_flat: Mixop,
    mixop_tree: &tree::Mixop,
) -> bool {
    arena_mixop
        .kind(mixop_flat)
        .matches_tree(arena_mixop, mixop_tree)
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
                MixopKind::Brack(atom_l_l, mixop_l, atom_l_r),
                MixopKind::Brack(atom_r_l, mixop_r, atom_r_r),
            ) => atom_l_l
                .node
                .cmp(&atom_r_l.node)
                .then_with(|| {
                    arena_mixop_l.kind(*mixop_l).cmp_by_inner(
                        arena_mixop_l,
                        arena_mixop_r.kind(*mixop_r),
                        arena_mixop_r,
                        pos,
                        compare_arg,
                    )
                })
                .then_with(|| atom_l_r.node.cmp(&atom_r_r.node)),
            // Infix: left form, operator, right form
            (
                MixopKind::Infix(mixop_l_l, atom_l, mixop_l_r),
                MixopKind::Infix(mixop_r_l, atom_r, mixop_r_r),
            ) => arena_mixop_l
                .kind(*mixop_l_l)
                .cmp_by_inner(
                    arena_mixop_l,
                    arena_mixop_r.kind(*mixop_r_l),
                    arena_mixop_r,
                    pos,
                    compare_arg,
                )
                .then_with(|| atom_l.node.cmp(&atom_r.node))
                .then_with(|| {
                    arena_mixop_l.kind(*mixop_l_r).cmp_by_inner(
                        arena_mixop_l,
                        arena_mixop_r.kind(*mixop_r_r),
                        arena_mixop_r,
                        pos,
                        compare_arg,
                    )
                }),
            // Sequences: common prefix first, then length
            (MixopKind::Seq(mixops_l), MixopKind::Seq(mixops_r)) => {
                let kinds_l = mixops_l.iter().map(|mixop| arena_mixop_l.kind(*mixop));
                let kinds_r = mixops_r.iter().map(|mixop| arena_mixop_r.kind(*mixop));
                for (kind_l, kind_r) in kinds_l.zip(kinds_r) {
                    let order =
                        kind_l.cmp_by_inner(arena_mixop_l, kind_r, arena_mixop_r, pos, compare_arg);
                    if order != Ordering::Equal {
                        return order;
                    }
                }
                mixops_l.len().cmp(&mixops_r.len())
            }
            // Different forms order by variant
            _ => self.tag().cmp(&kind_r.tag()),
        }
    }

    /// Compares this node and its children with a tree, ignoring atom spans.
    fn matches_tree(&self, arena_mixop: &MixopArena, mixop_tree: &tree::Mixop) -> bool {
        match (self, mixop_tree) {
            (MixopKind::Arg, tree::Mixop::Arg) => true,
            (MixopKind::Atom(atom_l), tree::Mixop::Atom(atom_r)) => atom_l.node == atom_r.node,
            (
                MixopKind::Brack(atom_l_l, mixop_l, atom_l_r),
                tree::Mixop::Brack(atom_r_l, mixop_r, atom_r_r),
            ) => {
                atom_l_l.node == atom_r_l.node
                    && atom_l_r.node == atom_r_r.node
                    && arena_mixop
                        .kind(*mixop_l)
                        .matches_tree(arena_mixop, mixop_r)
            }
            (
                MixopKind::Infix(mixop_l_l, atom_l, mixop_l_r),
                tree::Mixop::Infix(mixop_r_l, atom_r, mixop_r_r),
            ) => {
                atom_l.node == atom_r.node
                    && arena_mixop
                        .kind(*mixop_l_l)
                        .matches_tree(arena_mixop, mixop_r_l)
                    && arena_mixop
                        .kind(*mixop_l_r)
                        .matches_tree(arena_mixop, mixop_r_r)
            }
            (MixopKind::Seq(mixops_l), tree::Mixop::Seq(mixops_r)) => {
                mixops_l.len() == mixops_r.len()
                    && mixops_l.iter().zip(mixops_r).all(|(mixop_l, mixop_r)| {
                        arena_mixop
                            .kind(*mixop_l)
                            .matches_tree(arena_mixop, mixop_r)
                    })
            }
            _ => false,
        }
    }
}

impl<T> Mixfix<T> {
    /// Orders two cases as their expanded trees would order.
    ///
    /// Each case reads its mixop in its own `MixopArena`;
    /// `compare_arg` orders the arguments at each position both reach.
    pub fn cmp_by<U>(
        &self,
        arena_mixop: &MixopArena,
        mixfix_other: &Mixfix<U>,
        arena_mixop_other: &MixopArena,
        mut compare_arg: impl FnMut(&T, &U) -> Ordering,
    ) -> Ordering {
        cmp_by(arena_mixop, self.mixop, arena_mixop_other, mixfix_other.mixop, |pos| {
            compare_arg(&self.args[pos], &mixfix_other.args[pos])
        })
    }
}

// = Traversal

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

impl MixopKind {
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
            MixopKind::Brack(atom_l, mixop, atom_r) => {
                visit_piece(Piece::Atom(atom_l));
                arena_mixop
                    .kind(*mixop)
                    .visit_inner(arena_mixop, pos, visit_piece);
                visit_piece(Piece::Atom(atom_r));
            }
            MixopKind::Infix(mixop_l, atom, mixop_r) => {
                arena_mixop
                    .kind(*mixop_l)
                    .visit_inner(arena_mixop, pos, visit_piece);
                visit_piece(Piece::Atom(atom));
                arena_mixop
                    .kind(*mixop_r)
                    .visit_inner(arena_mixop, pos, visit_piece);
            }
            MixopKind::Seq(mixops) => {
                for mixop in mixops {
                    arena_mixop
                        .kind(*mixop)
                        .visit_inner(arena_mixop, pos, visit_piece);
                }
            }
        }
    }
}
