//! Tree representation of notation: children kept in place
//!
//! `Mixop` boxes lone children and keeps sequence elements inline,
//! owning its whole form.
//! Equality, ordering, and hashing read atom names, never atom spans,
//! and printing writes `%` at each argument position.

use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
};

use serde::{Deserialize, Serialize};

use crate::lang::{
    common::ds::set::IdSet,
    traits::{
        eq::SyntaxEq,
        free::FreeIds,
        print::{Print, Printer},
    },
};

use super::{AtomPhrase, MixopArena, MixopError, Piece, flat, print};

pub use super::mixfix::{MixfixRef, View};

/// An owned notation with an argument hole at each position.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Mixop {
    Arg,
    Atom(AtomPhrase),
    Brack(AtomPhrase, Box<Mixop>, AtomPhrase),
    Infix(Box<Mixop>, AtomPhrase, Box<Mixop>),
    Seq(Vec<Mixop>),
}

impl Mixop {
    /// Counts argument positions in notation order.
    pub fn arity(&self) -> usize {
        match self {
            Self::Arg => 1,
            Self::Atom(_) => 0,
            Self::Brack(_, mixop, _) => mixop.arity(),
            Self::Infix(mixop_l, _, mixop_r) => mixop_l.arity() + mixop_r.arity(),
            Self::Seq(mixops) => mixops.iter().map(Self::arity).sum(),
        }
    }

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

// = Comparison and traversal

impl Mixop {
    /// Orders two mixops by structure and atom names, lexicographically.
    ///
    /// Brackets compare the opening atom, the inner form, then the closing atom;
    /// infix compares the left form, the operator, then the right form;
    /// sequences compare the common prefix, then the length;
    /// different forms order by variant.
    /// Both walks visit equal prefixes, so they reach argument positions in step,
    /// and `compare_arg` orders the arguments at each position.
    pub(super) fn cmp_by(
        &self,
        mixop_other: &Self,
        mut compare_arg: impl FnMut(usize) -> Ordering,
    ) -> Ordering {
        let mut pos = 0;
        self.cmp_by_inner(mixop_other, &mut pos, &mut compare_arg)
    }

    /// Structural comparison, threading the position and the argument comparator.
    fn cmp_by_inner(
        &self,
        mixop_r: &Self,
        pos: &mut usize,
        compare_arg: &mut impl FnMut(usize) -> Ordering,
    ) -> Ordering {
        match (self, mixop_r) {
            // Arguments by the caller's comparator
            (Mixop::Arg, Mixop::Arg) => {
                let order = compare_arg(*pos);
                *pos += 1;
                order
            }
            // Atoms by name
            (Mixop::Atom(atom_l), Mixop::Atom(atom_r)) => atom_l.node.cmp(&atom_r.node),
            // Brackets: opening atom, inner form, closing atom
            (
                Mixop::Brack(atom_l_l, child_l, atom_l_r),
                Mixop::Brack(atom_r_l, child_r, atom_r_r),
            ) => atom_l_l
                .node
                .cmp(&atom_r_l.node)
                .then_with(|| child_l.cmp_by_inner(child_r, pos, compare_arg))
                .then_with(|| atom_l_r.node.cmp(&atom_r_r.node)),
            // Infix: left form, operator, right form
            (
                Mixop::Infix(child_l_l, atom_l, child_l_r),
                Mixop::Infix(child_r_l, atom_r, child_r_r),
            ) => child_l_l
                .cmp_by_inner(child_r_l, pos, compare_arg)
                .then_with(|| atom_l.node.cmp(&atom_r.node))
                .then_with(|| child_l_r.cmp_by_inner(child_r_r, pos, compare_arg)),
            // Sequences: common prefix first, then length
            (Mixop::Seq(elems_l), Mixop::Seq(elems_r)) => {
                let mixops_l = elems_l.iter();
                let mixops_r = elems_r.iter();
                for (mixop_l, mixop_r) in mixops_l.zip(mixops_r) {
                    let order = mixop_l.cmp_by_inner(mixop_r, pos, compare_arg);
                    if order != Ordering::Equal {
                        return order;
                    }
                }
                elems_l.len().cmp(&elems_r.len())
            }
            // Different forms order by variant
            _ => self.tag().cmp(&mixop_r.tag()),
        }
    }

    /// Visits atoms and argument positions in reading order.
    pub(super) fn visit<'a>(&'a self, mut visit_piece: impl FnMut(Piece<'a>)) {
        let mut pos = 0;
        self.visit_inner(&mut pos, &mut visit_piece);
    }

    /// Visits one mixop, threading the position.
    fn visit_inner<'a>(&'a self, pos: &mut usize, visit_piece: &mut impl FnMut(Piece<'a>)) {
        match self {
            Mixop::Arg => {
                visit_piece(Piece::Arg(*pos));
                *pos += 1;
            }
            Mixop::Atom(atom) => visit_piece(Piece::Atom(atom)),
            Mixop::Brack(atom_l, child, atom_r) => {
                visit_piece(Piece::Atom(atom_l));
                child.visit_inner(pos, visit_piece);
                visit_piece(Piece::Atom(atom_r));
            }
            Mixop::Infix(child_l, atom, child_r) => {
                child_l.visit_inner(pos, visit_piece);
                visit_piece(Piece::Atom(atom));
                child_r.visit_inner(pos, visit_piece);
            }
            Mixop::Seq(elems) => {
                for elem in elems {
                    elem.visit_inner(pos, visit_piece);
                }
            }
        }
    }
}

// = Equality, ordering, and hashing

impl PartialEq for Mixop {
    fn eq(&self, node_other: &Self) -> bool {
        self.cmp(node_other).is_eq()
    }
}

impl Eq for Mixop {}

impl Ord for Mixop {
    fn cmp(&self, node_other: &Self) -> Ordering {
        self.cmp_by(node_other, |_| Ordering::Equal)
    }
}

impl PartialOrd for Mixop {
    fn partial_cmp(&self, node_other: &Self) -> Option<Ordering> {
        Some(self.cmp(node_other))
    }
}

impl Hash for Mixop {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        // Hash the form first so different variants rarely collide
        self.tag().hash(hasher);
        match self {
            Self::Arg => {}
            Self::Atom(atom) => atom.node.hash(hasher),
            Self::Brack(atom_l, child, atom_r) => {
                atom_l.node.hash(hasher);
                child.hash(hasher);
                atom_r.node.hash(hasher);
            }
            Self::Infix(child_l, atom, child_r) => {
                child_l.hash(hasher);
                atom.node.hash(hasher);
                child_r.hash(hasher);
            }
            Self::Seq(elems) => elems.hash(hasher),
        }
    }
}

// = Syntax operations

impl SyntaxEq for Mixop {
    fn syntax_eq(&self, mixop_other: &Self) -> bool {
        self == mixop_other
    }
}

impl FreeIds for Mixop {
    fn free_ids(&self) -> IdSet {
        IdSet::new()
    }
}

// = Printing

impl Print for Mixop {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        print::tree_with(self, printer, |_, printer| printer.write("%"))
    }
}

/// Expands a handle into an owned notation, preserving atom spans.
pub fn from_flat(arena_mixop: &MixopArena, mixop: flat::Mixop) -> Mixop {
    match arena_mixop.kind(mixop) {
        flat::MixopKind::Arg => Mixop::Arg,
        flat::MixopKind::Atom(atom) => Mixop::Atom(atom.clone()),
        flat::MixopKind::Brack(atom_l, child, atom_r) => {
            Mixop::Brack(atom_l.clone(), Box::new(from_flat(arena_mixop, *child)), atom_r.clone())
        }
        flat::MixopKind::Infix(child_l, atom, child_r) => Mixop::Infix(
            Box::new(from_flat(arena_mixop, *child_l)),
            atom.clone(),
            Box::new(from_flat(arena_mixop, *child_r)),
        ),
        flat::MixopKind::Seq(elems) => Mixop::Seq(
            elems
                .iter()
                .map(|elem| from_flat(arena_mixop, *elem))
                .collect(),
        ),
    }
}

/// Interns an owned notation in the target arena.
pub fn into_flat(
    arena_mixop: &mut MixopArena,
    mixop_tree: Mixop,
) -> Result<flat::Mixop, MixopError> {
    arena_mixop.intern(&mixop_tree)
}
