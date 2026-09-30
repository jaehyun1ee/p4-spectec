//! Append-only storage for the notation shapes of one specification
//!
//! A shape handle is valid only in the arena that issued it.
//! `intern_notation` and `split_notation` intern a `Mixfix` children first,
//! left to right, so argument positions keep their notation order;
//! `fill` expands a shape back into a `Mixfix` with the given arguments.

use std::num::TryFromIntError;

use thiserror::Error;

use crate::lang::data::intern::{CanonId, CanonInterner};

use super::{
    mixop::ArityMismatch,
    shape::{Shape, ShapeKind},
    tree::Mixfix,
};

// = Errors

/// A failure interning a shape.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum ShapeError {
    /// The arena ran out of 32-bit handles.
    #[error("shape arena index overflow")]
    IndexOverflow,
}

// - Index overflow

impl From<TryFromIntError> for ShapeError {
    fn from(_: TryFromIntError) -> Self {
        Self::IndexOverflow
    }
}

// = Arena storage

/// Storage for notation shapes, shared by every value built from them.
#[derive(Debug, Default)]
pub struct ShapeArena {
    /// Shapes, with canonical identities that ignore atom spans.
    shapes: CanonInterner<ShapeKind>,
}

impl ShapeArena {
    // - Construction

    /// An empty arena.
    pub fn new() -> Self {
        Self::default()
    }

    // - Interning

    /// Interns a notation's shape and counts its argument positions.
    ///
    /// Atoms are copied; the arguments themselves are not read.
    pub fn intern_notation<T>(&mut self, mixfix: &Mixfix<T>) -> Result<(Shape, usize), ShapeError> {
        let mut arity = 0;
        let shape = self.intern_borrowed(mixfix, &mut arity)?;
        Ok((shape, arity))
    }

    /// Interns the nodes of a borrowed notation, children first.
    fn intern_borrowed<T>(
        &mut self,
        mixfix: &Mixfix<T>,
        arity: &mut usize,
    ) -> Result<Shape, ShapeError> {
        // Children get their canonical identities before the parent is hashed
        let kind = match mixfix {
            Mixfix::Arg(_) => {
                *arity += 1;
                ShapeKind::Arg
            }
            Mixfix::Atom(atom) => ShapeKind::Atom(atom.clone()),
            Mixfix::Brack(atom_l, mixfix_inner, atom_r) => {
                let shape_inner = self.intern_borrowed(mixfix_inner, arity)?;
                ShapeKind::Brack(atom_l.clone(), shape_inner, atom_r.clone())
            }
            Mixfix::Infix(mixfix_l, atom, mixfix_r) => {
                let shape_l = self.intern_borrowed(mixfix_l, arity)?;
                let shape_r = self.intern_borrowed(mixfix_r, arity)?;
                ShapeKind::Infix(shape_l, atom.clone(), shape_r)
            }
            Mixfix::Seq(mixfixes) => ShapeKind::Seq(
                mixfixes
                    .iter()
                    .map(|mixfix| self.intern_borrowed(mixfix, arity))
                    .collect::<Result<_, _>>()?,
            ),
        };
        Ok(self.shapes.intern(kind, &())?)
    }

    /// Interns a notation's shape and moves its arguments out in notation order.
    pub(crate) fn split_notation<T>(
        &mut self,
        mixfix: Mixfix<T>,
    ) -> Result<(Shape, Vec<T>), ShapeError> {
        let mut args = Vec::new();
        let shape = self.intern_owned(mixfix, &mut args)?;
        Ok((shape, args))
    }

    /// Interns the nodes of an owned notation, moving atoms and arguments.
    fn intern_owned<T>(
        &mut self,
        mixfix: Mixfix<T>,
        args: &mut Vec<T>,
    ) -> Result<Shape, ShapeError> {
        // Children get their canonical identities before the parent is hashed
        let kind = match mixfix {
            Mixfix::Arg(arg) => {
                args.push(arg);
                ShapeKind::Arg
            }
            Mixfix::Atom(atom) => ShapeKind::Atom(atom),
            Mixfix::Brack(atom_l, mixfix_inner, atom_r) => {
                let shape_inner = self.intern_owned(*mixfix_inner, args)?;
                ShapeKind::Brack(atom_l, shape_inner, atom_r)
            }
            Mixfix::Infix(mixfix_l, atom, mixfix_r) => {
                let shape_l = self.intern_owned(*mixfix_l, args)?;
                let shape_r = self.intern_owned(*mixfix_r, args)?;
                ShapeKind::Infix(shape_l, atom, shape_r)
            }
            Mixfix::Seq(mixfixes) => ShapeKind::Seq(
                mixfixes
                    .into_iter()
                    .map(|mixfix| self.intern_owned(mixfix, args))
                    .collect::<Result<_, _>>()?,
            ),
        };
        Ok(self.shapes.intern(kind, &())?)
    }

    // - Lookup

    /// The node behind a shape handle.
    pub fn kind(&self, shape: Shape) -> &ShapeKind {
        self.shapes.get(shape)
    }

    /// The canonical identity of a shape, ignoring atom spans.
    pub fn canon_id(&self, shape: Shape) -> CanonId<ShapeKind> {
        self.shapes.canon_id(shape)
    }

    /// Whether two shapes have the same structure and atom names.
    pub fn canon_eq(&self, shape_l: Shape, shape_r: Shape) -> bool {
        self.canon_id(shape_l) == self.canon_id(shape_r)
    }

    // - Notation comparison

    /// Whether a shape has a notation's structure and atom names.
    ///
    /// Atom spans and the notation's arguments are not compared.
    pub fn eq_notation<T>(&self, shape: Shape, mixfix: &Mixfix<T>) -> bool {
        match (self.kind(shape), mixfix) {
            // A position matches any argument
            (ShapeKind::Arg, Mixfix::Arg(_)) => true,
            // Atoms match by name
            (ShapeKind::Atom(atom_l), Mixfix::Atom(atom_r)) => atom_l.node == atom_r.node,
            // Brackets match both atoms and the inner shape
            (
                ShapeKind::Brack(atom_l_l, shape_inner, atom_l_r),
                Mixfix::Brack(atom_r_l, mixfix_inner, atom_r_r),
            ) => {
                atom_l_l.node == atom_r_l.node
                    && self.eq_notation(*shape_inner, mixfix_inner)
                    && atom_l_r.node == atom_r_r.node
            }
            // Infix shapes match both sides and the operator
            (
                ShapeKind::Infix(shape_l, atom_l, shape_r),
                Mixfix::Infix(mixfix_l, atom_r, mixfix_r),
            ) => {
                self.eq_notation(*shape_l, mixfix_l)
                    && atom_l.node == atom_r.node
                    && self.eq_notation(*shape_r, mixfix_r)
            }
            // Sequences match elementwise at equal length
            (ShapeKind::Seq(shapes), Mixfix::Seq(mixfixes)) => {
                shapes.len() == mixfixes.len()
                    && shapes
                        .iter()
                        .zip(mixfixes)
                        .all(|(shape, mixfix)| self.eq_notation(*shape, mixfix))
            }
            // Different node kinds differ
            _ => false,
        }
    }

    // - Expansion

    /// Expands a shape into a notation, filling positions left to right.
    pub fn fill<T>(
        &self,
        shape: Shape,
        args: impl IntoIterator<Item = T>,
    ) -> Result<Mixfix<T>, ArityMismatch> {
        // Consume arguments in notation order; leftovers are too many
        let mut args = args.into_iter();
        let mixfix = self.fill_inner(shape, &mut args)?;
        if args.next().is_some() { Err(ArityMismatch::ArgumentCountTooMany) } else { Ok(mixfix) }
    }

    /// Expands one node, taking arguments from the iterator.
    fn fill_inner<T>(
        &self,
        shape: Shape,
        args: &mut impl Iterator<Item = T>,
    ) -> Result<Mixfix<T>, ArityMismatch> {
        Ok(match self.kind(shape) {
            // A hole takes the next argument
            ShapeKind::Arg => Mixfix::Arg(args.next().ok_or(ArityMismatch::ArgumentCountTooFew)?),
            // Atoms are copied with their spans
            ShapeKind::Atom(atom) => Mixfix::Atom(atom.clone()),
            // Compound shapes fill their parts left to right
            ShapeKind::Brack(atom_l, shape_inner, atom_r) => Mixfix::Brack(
                atom_l.clone(),
                Box::new(self.fill_inner(*shape_inner, args)?),
                atom_r.clone(),
            ),
            ShapeKind::Infix(shape_l, atom, shape_r) => Mixfix::Infix(
                Box::new(self.fill_inner(*shape_l, args)?),
                atom.clone(),
                Box::new(self.fill_inner(*shape_r, args)?),
            ),
            ShapeKind::Seq(shapes) => Mixfix::Seq(
                shapes
                    .iter()
                    .map(|shape| self.fill_inner(*shape, args))
                    .collect::<Result<_, _>>()?,
            ),
        })
    }
}
