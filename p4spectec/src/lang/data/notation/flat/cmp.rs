//! Comparison of flat notation
//!
//! Canonical equality uses child canonical identities.
//! Structural comparison resolves handles and visits arguments in notation order;
//! `matches_tree` compares a parsed tree pattern without allocating a tree.

use std::cmp::Ordering;

use crate::lang::data::intern::{CanonEq, CanonInterner};

use super::super::tree;
use super::{Mixfix, Mixop, MixopArena, MixopKind};

// = Canonical equality

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

// = Structural comparison

impl Mixop {
    /// Orders two mixops by structure and atom names, lexicographically.
    ///
    /// Brackets compare the opening atom, the inner form, then the closing atom;
    /// infix compares the left form, the operator, then the right form;
    /// sequences compare the common prefix, then the length;
    /// different forms order by variant.
    /// Both walks visit equal prefixes, so they reach argument positions in step,
    /// and `compare_arg` orders the arguments at each position.
    pub(in crate::lang::data::notation) fn cmp_by(
        self,
        arena_mixop: &MixopArena,
        mixop_r: Mixop,
        mut compare_arg: impl FnMut(usize) -> Ordering,
    ) -> Ordering {
        let mut pos = 0;
        self.cmp_by_inner(arena_mixop, mixop_r, &mut pos, &mut compare_arg)
    }

    /// Structural comparison, threading the position and the argument comparator.
    fn cmp_by_inner(
        self,
        arena_mixop: &MixopArena,
        mixop_r: Self,
        pos: &mut usize,
        compare_arg: &mut impl FnMut(usize) -> Ordering,
    ) -> Ordering {
        match (arena_mixop.kind(self), arena_mixop.kind(mixop_r)) {
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
                .then_with(|| mixop_l.cmp_by_inner(arena_mixop, *mixop_r, pos, compare_arg))
                .then_with(|| atom_l_r.node.cmp(&atom_r_r.node)),
            // Infix: left form, operator, right form
            (
                MixopKind::Infix(mixop_l_l, atom_l, mixop_l_r),
                MixopKind::Infix(mixop_r_l, atom_r, mixop_r_r),
            ) => mixop_l_l
                .cmp_by_inner(arena_mixop, *mixop_r_l, pos, compare_arg)
                .then_with(|| atom_l.node.cmp(&atom_r.node))
                .then_with(|| mixop_l_r.cmp_by_inner(arena_mixop, *mixop_r_r, pos, compare_arg)),
            // Sequences: common prefix first, then length
            (MixopKind::Seq(mixops_l), MixopKind::Seq(mixops_r)) => {
                for (mixop_l, mixop_r) in mixops_l.iter().zip(mixops_r) {
                    let order = mixop_l.cmp_by_inner(arena_mixop, *mixop_r, pos, compare_arg);
                    if order != Ordering::Equal {
                        return order;
                    }
                }
                mixops_l.len().cmp(&mixops_r.len())
            }
            // Different forms order by variant
            (kind_l, kind_r) => kind_l.tag().cmp(&kind_r.tag()),
        }
    }

    /// Compares this node and its children with a tree, ignoring atom spans.
    pub(crate) fn matches_tree(self, arena_mixop: &MixopArena, mixop_tree: &tree::Mixop) -> bool {
        match (arena_mixop.kind(self), mixop_tree) {
            (MixopKind::Arg, tree::Mixop::Arg) => true,
            (MixopKind::Atom(atom_l), tree::Mixop::Atom(atom_r)) => atom_l.node == atom_r.node,
            (
                MixopKind::Brack(atom_l_l, mixop_l, atom_l_r),
                tree::Mixop::Brack(atom_r_l, mixop_r, atom_r_r),
            ) => {
                atom_l_l.node == atom_r_l.node
                    && atom_l_r.node == atom_r_r.node
                    && mixop_l.matches_tree(arena_mixop, mixop_r)
            }
            (
                MixopKind::Infix(mixop_l_l, atom_l, mixop_l_r),
                tree::Mixop::Infix(mixop_r_l, atom_r, mixop_r_r),
            ) => {
                atom_l.node == atom_r.node
                    && mixop_l_l.matches_tree(arena_mixop, mixop_r_l)
                    && mixop_l_r.matches_tree(arena_mixop, mixop_r_r)
            }
            (MixopKind::Seq(mixops_l), tree::Mixop::Seq(mixops_r)) => {
                mixops_l.len() == mixops_r.len()
                    && mixops_l
                        .iter()
                        .zip(mixops_r)
                        .all(|(mixop_l, mixop_r)| mixop_l.matches_tree(arena_mixop, mixop_r))
            }
            _ => false,
        }
    }
}

impl<T> Mixfix<T> {
    /// Orders two cases as their expanded trees would order.
    ///
    /// Both cases must belong to `arena_mixop`;
    /// `compare_arg` orders the arguments at each position both reach.
    pub fn cmp_by<U>(
        &self,
        arena_mixop: &MixopArena,
        mixfix_other: &Mixfix<U>,
        mut compare_arg: impl FnMut(&T, &U) -> Ordering,
    ) -> Ordering {
        self.mixop.cmp_by(arena_mixop, mixfix_other.mixop, |pos| {
            compare_arg(&self.args[pos], &mixfix_other.args[pos])
        })
    }
}
