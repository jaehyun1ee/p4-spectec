//! Append-only storage for the notation shapes of one specification
//!
//! A shape handle is valid only in the arena that issued it.
//! `intern_notation` and `split_notation` intern a `Mixfix` children first,
//! left to right, so argument positions keep their notation order;
//! `fill` expands a shape back into a `Mixfix` with the given arguments;
//! comparison and expansion go through `walk`.

use crate::lang::data::intern::{CanonId, CanonInterner};

use super::{
    error::{ArityMismatch, ShapeError},
    handle::{Shape, ShapeKind},
    tree::Mixfix,
    walk,
};

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
                ShapeKind::Arg(())
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
                ShapeKind::Arg(())
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
        walk::eq_by(self, self.kind(shape), &(), mixfix, |(), _| true)
    }

    // - Expansion

    /// Expands a shape into a notation, filling positions left to right.
    ///
    /// Fails when `args` has fewer or more items than the shape's positions.
    pub fn fill<T>(
        &self,
        shape: Shape,
        args: impl IntoIterator<Item = T>,
    ) -> Result<Mixfix<T>, ArityMismatch> {
        let kind = self.kind(shape);
        let arity = walk::arity(self, kind);
        // Take exactly one argument per position; leftovers are too many
        let mut args = args.into_iter();
        let args_taken: Vec<T> = args.by_ref().take(arity).collect();
        if args_taken.len() < arity {
            return Err(ArityMismatch::ArgumentCountTooFew);
        }
        if args.next().is_some() {
            return Err(ArityMismatch::ArgumentCountTooMany);
        }
        let mut args_taken = args_taken.into_iter();
        Ok(walk::to_tree(self, kind, |()| args_taken.next().expect("one argument per position")))
    }
}
