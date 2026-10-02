//! Notations interned as shapes, kept apart from their arguments
//!
//! `LEFT n` splits into the shape of `LEFT %` and the arguments `[n]`.
//! `from_mixfix` interns a filled notation and moves its arguments out
//! in notation order; comparison, printing, and expansion walk the shape
//! and take arguments from the list as positions are reached,
//! so a split always has one argument per position of its shape.
//! Case values are splits of values.

use std::{cmp::Ordering, fmt};

use crate::lang::traits::print::Printer;

use super::{
    arena::ShapeArena,
    error::ShapeError,
    handle::Shape,
    tree::{Mixfix, Mixop},
    walk,
};

// = Splits

/// A notation interned as a shape, with one argument per position, in order.
///
/// The shape belongs to the `ShapeArena` the split was built in.
/// Exact equality compares the shape handle, so atom spans count;
/// fields are private so every split keeps its argument count.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Split<T> {
    /// The notation with argument positions.
    shape: Shape,
    /// Arguments in notation order, one per position.
    args: Vec<T>,
}

impl<T> Split<T> {
    // - Construction

    /// Interns a filled notation, keeping its arguments in notation order.
    pub fn from_mixfix(
        arena_shape: &mut ShapeArena,
        mixfix: Mixfix<T>,
    ) -> Result<Self, ShapeError> {
        let (shape, args) = arena_shape.split_notation(mixfix)?;
        Ok(Self { shape, args })
    }

    // - Access

    /// The notation handle.
    pub fn shape(&self) -> Shape {
        self.shape
    }

    /// The arguments in notation order.
    pub fn args(&self) -> &[T] {
        &self.args
    }

    // - Notation comparison

    /// Whether the split has the shape's structure and atom names.
    ///
    /// Compares canonical identities, so atom spans are not compared;
    /// `shape` must belong to `arena_shape`.
    pub fn matches_shape(&self, arena_shape: &ShapeArena, shape: Shape) -> bool {
        arena_shape.canon_eq(self.shape, shape)
    }

    /// Whether the split has a notation's structure and atom names.
    ///
    /// Atom spans and arguments are not compared.
    pub fn eq_shape<A>(&self, arena_shape: &ShapeArena, mixfix: &Mixfix<A>) -> bool {
        arena_shape.eq_notation(self.shape, mixfix)
    }

    // - Expansion

    /// Expands the split into its filled notation.
    pub fn to_mixfix(&self, arena_shape: &ShapeArena) -> Mixfix<T>
    where
        T: Clone,
    {
        let mut args = self.args.iter();
        walk::to_tree(arena_shape, arena_shape.kind(self.shape), |()| {
            args.next().expect("a split fills every position").clone()
        })
    }

    /// Expands the notation with unfilled argument positions.
    pub fn to_mixop(&self, arena_shape: &ShapeArena) -> Mixop {
        walk::to_tree(arena_shape, arena_shape.kind(self.shape), |()| ())
    }

    // - Printing

    /// Writes the filled notation as `Mixfix::print_with` does.
    pub fn print_with(
        &self,
        arena_shape: &ShapeArena,
        printer: &mut Printer<'_>,
        mut print_arg: impl FnMut(&T, &mut Printer<'_>) -> fmt::Result,
    ) -> fmt::Result {
        let mut args = self.args.iter();
        walk::print_with(arena_shape, arena_shape.kind(self.shape), printer, |(), printer| {
            print_arg(args.next().expect("a split fills every position"), printer)
        })
    }

    // - Comparison

    /// Orders two splits as their filled notations would order.
    ///
    /// Each split reads its shape in its own `ShapeArena`;
    /// arguments compare with `compare_arg` at their positions.
    /// Both walks visit equal prefixes, so they reach positions in step.
    pub fn cmp_by(
        &self,
        arena_shape: &ShapeArena,
        split_other: &Self,
        arena_shape_other: &ShapeArena,
        mut compare_arg: impl FnMut(&T, &T) -> Ordering,
    ) -> Ordering {
        let mut args_l = self.args.iter();
        let mut args_r = split_other.args.iter();
        walk::cmp_by(
            arena_shape,
            arena_shape.kind(self.shape),
            arena_shape_other,
            arena_shape_other.kind(split_other.shape),
            |(), ()| {
                let arg_l = args_l.next().expect("a split fills every position");
                let arg_r = args_r.next().expect("a split fills every position");
                compare_arg(arg_l, arg_r)
            },
        )
    }
}
