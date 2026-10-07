//! Tree representation of notation: children kept in place
//!
//! `Mixop` boxes lone children and keeps sequence elements inline,
//! owning its whole form.
//! Equality, ordering, and hashing read atom names, never atom spans,
//! and printing writes `%` at each argument position.

use std::{
    cmp::Ordering,
    hash::{Hash, Hasher},
    rc::Rc,
};

use serde::{Deserialize, Serialize};

use crate::lang::{
    common::{ds::set::IdSet, source::Span},
    traits::{at::At, cmp::SyntaxCmp, eq::SyntaxEq, free::FreeIds},
};

use super::{AtomPhrase, Mixfix, MixopArena, MixopError, Piece, flat};

pub use super::view::{MixfixRef, MixfixView};

// = Notation forms

/// An owned notation with an argument hole at each position.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Mixop {
    Arg,
    Atom(AtomPhrase),
    Brack(AtomPhrase, Box<Mixop>, AtomPhrase),
    Infix(Box<Mixop>, AtomPhrase, Box<Mixop>),
    Seq(Vec<Mixop>),
}

// = Structural properties

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
}

// = Equality, ordering, and hashing

impl Mixop {
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
                Mixop::Brack(atom_l_l, mixop_l, atom_l_r),
                Mixop::Brack(atom_r_l, mixop_r, atom_r_r),
            ) => atom_l_l
                .node
                .cmp(&atom_r_l.node)
                .then_with(|| mixop_l.cmp_by_inner(mixop_r, pos, compare_arg))
                .then_with(|| atom_l_r.node.cmp(&atom_r_r.node)),
            // Infix: left form, operator, right form
            (
                Mixop::Infix(mixop_l_l, atom_l, mixop_l_r),
                Mixop::Infix(mixop_r_l, atom_r, mixop_r_r),
            ) => mixop_l_l
                .cmp_by_inner(mixop_r_l, pos, compare_arg)
                .then_with(|| atom_l.node.cmp(&atom_r.node))
                .then_with(|| mixop_l_r.cmp_by_inner(mixop_r_r, pos, compare_arg)),
            // Sequences: common prefix first, then length
            (Mixop::Seq(mixops_l), Mixop::Seq(mixops_r)) => {
                for (mixop_l, mixop_r) in mixops_l.iter().zip(mixops_r) {
                    let order = mixop_l.cmp_by_inner(mixop_r, pos, compare_arg);
                    if order != Ordering::Equal {
                        return order;
                    }
                }
                mixops_l.len().cmp(&mixops_r.len())
            }
            // Different forms order by variant
            _ => self.tag().cmp(&mixop_r.tag()),
        }
    }
}

impl PartialEq for Mixop {
    fn eq(&self, mixop_other: &Self) -> bool {
        self.cmp(mixop_other).is_eq()
    }
}

impl Eq for Mixop {}

impl Ord for Mixop {
    fn cmp(&self, mixop_other: &Self) -> Ordering {
        self.cmp_by(mixop_other, |_| Ordering::Equal)
    }
}

impl PartialOrd for Mixop {
    fn partial_cmp(&self, mixop_other: &Self) -> Option<Ordering> {
        Some(self.cmp(mixop_other))
    }
}

impl Hash for Mixop {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        // Hash the form first so different variants rarely collide
        self.tag().hash(hasher);
        match self {
            Self::Arg => {}
            Self::Atom(atom) => atom.node.hash(hasher),
            Self::Brack(atom_l, mixop, atom_r) => {
                atom_l.node.hash(hasher);
                mixop.hash(hasher);
                atom_r.node.hash(hasher);
            }
            Self::Infix(mixop_l, atom, mixop_r) => {
                mixop_l.hash(hasher);
                atom.node.hash(hasher);
                mixop_r.hash(hasher);
            }
            Self::Seq(mixops) => mixops.hash(hasher),
        }
    }
}

impl SyntaxEq for Mixop {
    fn syntax_eq(&self, mixop_other: &Self) -> bool {
        self == mixop_other
    }
}

impl<T> Mixfix<Rc<Mixop>, T> {
    /// Whether two mixfixes have the same structure and atom names.
    ///
    /// Atom spans and arguments are not compared.
    pub fn eq_mixop<U>(&self, mixfix_other: &Mixfix<Rc<Mixop>, U>) -> bool {
        self.mixop.as_ref() == mixfix_other.mixop.as_ref()
    }

    /// Orders two mixfixes as the walk of their mixops meets atoms and arguments.
    ///
    /// Atoms compare by name; `compare_arg` orders the arguments
    /// at each position both mixops reach.
    pub fn cmp_by<U>(
        &self,
        mixfix_other: &Mixfix<Rc<Mixop>, U>,
        mut compare_arg: impl FnMut(&T, &U) -> Ordering,
    ) -> Ordering {
        self.mixop.cmp_by(mixfix_other.mixop.as_ref(), |pos| {
            compare_arg(&self.args[pos], &mixfix_other.args[pos])
        })
    }
}

impl<T: SyntaxEq> SyntaxEq for Mixfix<Rc<Mixop>, T> {
    fn syntax_eq(&self, mixfix_other: &Self) -> bool {
        self.eq_mixop(mixfix_other)
            && self
                .args
                .iter()
                .zip(&mixfix_other.args)
                .all(|(arg_l, arg_r)| arg_l.syntax_eq(arg_r))
    }
}

impl<T: SyntaxCmp> SyntaxCmp for Mixfix<Rc<Mixop>, T> {
    fn syntax_cmp(&self, mixfix_other: &Self) -> Ordering {
        self.cmp_by(mixfix_other, SyntaxCmp::syntax_cmp)
    }
}

// = Traversal

impl Mixop {
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
            Mixop::Brack(atom_l, mixop, atom_r) => {
                visit_piece(Piece::Atom(atom_l));
                mixop.visit_inner(pos, visit_piece);
                visit_piece(Piece::Atom(atom_r));
            }
            Mixop::Infix(mixop_l, atom, mixop_r) => {
                mixop_l.visit_inner(pos, visit_piece);
                visit_piece(Piece::Atom(atom));
                mixop_r.visit_inner(pos, visit_piece);
            }
            Mixop::Seq(mixops) => {
                for mixop in mixops {
                    mixop.visit_inner(pos, visit_piece);
                }
            }
        }
    }
}

impl FreeIds for Mixop {
    fn free_ids(&self) -> IdSet {
        IdSet::new()
    }
}

// - Source locations

impl<T: At> At for Mixfix<Rc<Mixop>, T> {
    fn at(&self) -> Span {
        // Cover atoms and arguments, so empty sequences add no default span
        let mut spans = Vec::new();
        self.mixop.visit(|piece| match piece {
            Piece::Atom(atom) => spans.push(atom.at()),
            Piece::Arg(pos) => spans.push(self.args[pos].at()),
        });
        spans.at()
    }
}

// = Arena conversion

// - Entry points

/// Expands a handle into an owned notation, preserving atom spans.
pub fn from_flat(arena_mixop: &MixopArena, mixop: flat::Mixop) -> Mixop {
    Mixop::from_flat(arena_mixop, mixop)
}

/// Interns an owned notation in the target arena.
pub fn into_flat(arena_mixop: &mut MixopArena, mixop: Mixop) -> Result<flat::Mixop, MixopError> {
    mixop.into_flat(arena_mixop)
}

// - Bodies

impl Mixop {
    /// Copies atoms and expands child handles in notation order.
    fn from_flat(arena_mixop: &MixopArena, mixop: flat::Mixop) -> Self {
        match arena_mixop.kind(mixop) {
            flat::MixopKind::Arg => Self::Arg,
            flat::MixopKind::Atom(atom) => Self::Atom(atom.clone()),
            flat::MixopKind::Brack(atom_l, mixop, atom_r) => Self::Brack(
                atom_l.clone(),
                Box::new(Self::from_flat(arena_mixop, *mixop)),
                atom_r.clone(),
            ),
            flat::MixopKind::Infix(mixop_l, atom, mixop_r) => Self::Infix(
                Box::new(Self::from_flat(arena_mixop, *mixop_l)),
                atom.clone(),
                Box::new(Self::from_flat(arena_mixop, *mixop_r)),
            ),
            flat::MixopKind::Seq(mixops) => Self::Seq(
                mixops
                    .iter()
                    .map(|mixop| Self::from_flat(arena_mixop, *mixop))
                    .collect(),
            ),
        }
    }

    /// Moves atoms into the arena, interning children before their parent.
    fn into_flat(self, arena_mixop: &mut MixopArena) -> Result<flat::Mixop, MixopError> {
        let kind = match self {
            Self::Arg => flat::MixopKind::Arg,
            Self::Atom(atom) => flat::MixopKind::Atom(atom),
            Self::Brack(atom_l, mixop, atom_r) => {
                let mixop = mixop.into_flat(arena_mixop)?;
                flat::MixopKind::Brack(atom_l, mixop, atom_r)
            }
            Self::Infix(mixop_l, atom, mixop_r) => {
                let mixop_l = mixop_l.into_flat(arena_mixop)?;
                let mixop_r = mixop_r.into_flat(arena_mixop)?;
                flat::MixopKind::Infix(mixop_l, atom, mixop_r)
            }
            Self::Seq(mixops) => flat::MixopKind::Seq(
                mixops
                    .into_iter()
                    .map(|mixop| mixop.into_flat(arena_mixop))
                    .collect::<Result<_, _>>()?,
            ),
        };
        arena_mixop.intern_kind(kind)
    }

    /// Copies atoms into the arena, interning children before their parent.
    pub(super) fn to_flat(&self, arena_mixop: &mut MixopArena) -> Result<flat::Mixop, MixopError> {
        let kind = match self {
            Self::Arg => flat::MixopKind::Arg,
            Self::Atom(atom) => flat::MixopKind::Atom(atom.clone()),
            Self::Brack(atom_l, mixop, atom_r) => {
                let mixop = mixop.to_flat(arena_mixop)?;
                flat::MixopKind::Brack(atom_l.clone(), mixop, atom_r.clone())
            }
            Self::Infix(mixop_l, atom, mixop_r) => {
                let mixop_l = mixop_l.to_flat(arena_mixop)?;
                let mixop_r = mixop_r.to_flat(arena_mixop)?;
                flat::MixopKind::Infix(mixop_l, atom.clone(), mixop_r)
            }
            Self::Seq(mixops) => flat::MixopKind::Seq(
                mixops
                    .iter()
                    .map(|mixop| mixop.to_flat(arena_mixop))
                    .collect::<Result<_, _>>()?,
            ),
        };
        arena_mixop.intern_kind(kind)
    }
}
