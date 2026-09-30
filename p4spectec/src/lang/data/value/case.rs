//! Case values as a notation shape and arguments in notation order
//!
//! `LEFT n` is stored as the shape of `LEFT _` and the argument list `[n]`.
//! Constructors intern the notation into the arena's `ShapeArena`,
//! or take a shape interned while preparing (`from_shape`);
//! comparisons, printing, and serialization walk the shape (`walk`)
//! and take arguments from the list as positions are reached,
//! so a case always has one argument per position of its shape.

use std::{cmp::Ordering, fmt};

use super::{Value, ValueError};
use crate::lang::{
    data::notation::{Mixfix, Mixop, Shape, ShapeArena, walk},
    traits::print::Printer,
};

// = Case values

/// A variant case: a shape handle and one argument per position.
///
/// The shape belongs to the `ShapeArena` of the arena holding the case.
/// Exact equality compares the shape handle, so atom spans count;
/// canonical identity and syntax comparison ignore them.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ValueCase {
    /// The notation with argument positions.
    shape: Shape,
    /// Arguments in notation order, one per position.
    args: Vec<Value>,
}

impl ValueCase {
    // - Construction

    /// Interns a filled notation, keeping its arguments in notation order.
    pub fn from_mixfix(shapes: &mut ShapeArena, mixfix: Mixfix<Value>) -> Result<Self, ValueError> {
        let (shape, args) = shapes.split_notation(mixfix)?;
        Ok(Self { shape, args })
    }

    /// Interns a notation and fills its positions with `args`.
    ///
    /// Fails when `args` has a different length than the notation's arity.
    pub fn from_notation<T>(
        shapes: &mut ShapeArena,
        mixfix: &Mixfix<T>,
        args: Vec<Value>,
    ) -> Result<Self, ValueError> {
        let (shape, arity) = shapes.intern_notation(mixfix)?;
        Self::from_shape(shape, arity, args)
    }

    /// Fills an interned shape of the given arity with `args`.
    ///
    /// Fails when `args` has a different length than `arity`.
    pub fn from_shape(shape: Shape, arity: usize, args: Vec<Value>) -> Result<Self, ValueError> {
        if args.len() != arity {
            return Err(ValueError::CountMismatch { expected: arity, actual: args.len() });
        }
        Ok(Self { shape, args })
    }

    // - Access

    /// The notation handle.
    pub fn shape(&self) -> Shape {
        self.shape
    }

    /// The arguments in notation order.
    pub fn args(&self) -> &[Value] {
        &self.args
    }

    // - Notation comparison

    /// Whether the case has the shape's structure and atom names.
    ///
    /// Compares canonical identities, so atom spans are not compared;
    /// `shape` must belong to `shapes`.
    pub fn matches_shape(&self, shapes: &ShapeArena, shape: Shape) -> bool {
        shapes.canon_eq(self.shape, shape)
    }

    /// Whether the case has a notation's structure and atom names.
    ///
    /// Atom spans and arguments are not compared.
    pub fn eq_shape<T>(&self, shapes: &ShapeArena, mixfix: &Mixfix<T>) -> bool {
        shapes.eq_notation(self.shape, mixfix)
    }

    // - Expansion

    /// Expands the case into its filled notation.
    pub fn to_mixfix(&self, shapes: &ShapeArena) -> Mixfix<Value> {
        let mut args = self.args.iter();
        walk::to_tree(shapes, shapes.kind(self.shape), |()| {
            *args.next().expect("a case fills every position")
        })
    }

    /// Expands the notation with unfilled argument positions.
    pub fn to_mixop(&self, shapes: &ShapeArena) -> Mixop {
        walk::to_tree(shapes, shapes.kind(self.shape), |()| ())
    }

    // - Printing

    /// Writes the filled notation as `Mixfix::print_with` does.
    pub fn print_with(
        &self,
        shapes: &ShapeArena,
        printer: &mut Printer<'_>,
        mut print_arg: impl FnMut(&Value, &mut Printer<'_>) -> fmt::Result,
    ) -> fmt::Result {
        let mut args = self.args.iter();
        walk::print_with(shapes, shapes.kind(self.shape), printer, |(), printer| {
            print_arg(args.next().expect("a case fills every position"), printer)
        })
    }

    // - Syntax comparison

    /// Orders two cases as their filled notations would order.
    ///
    /// Each case reads its shape in its own `ShapeArena`;
    /// arguments compare with `compare_arg` at their positions.
    /// Both walks visit equal prefixes, so they reach positions in step.
    pub(super) fn cmp_by(
        &self,
        shapes: &ShapeArena,
        value_case_other: &Self,
        shapes_other: &ShapeArena,
        mut compare_arg: impl FnMut(&Value, &Value) -> Ordering,
    ) -> Ordering {
        let mut args_l = self.args.iter();
        let mut args_r = value_case_other.args.iter();
        walk::cmp_by(
            shapes,
            shapes.kind(self.shape),
            shapes_other,
            shapes_other.kind(value_case_other.shape),
            |(), ()| {
                let value_l = args_l.next().expect("a case fills every position");
                let value_r = args_r.next().expect("a case fills every position");
                compare_arg(value_l, value_r)
            },
        )
    }
}
