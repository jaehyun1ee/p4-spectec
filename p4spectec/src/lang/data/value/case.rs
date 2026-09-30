//! Case values as a notation shape and arguments in notation order
//!
//! `LEFT n` is stored as the shape of `LEFT _` and the argument list `[n]`.
//! Constructors intern the notation into the arena's `ShapeArena`,
//! or take a shape interned while preparing (`from_shape`);
//! comparisons, printing, and serialization walk the shape
//! and take arguments from the list as positions are reached.

use std::{cmp::Ordering, fmt, slice};

use super::{Value, ValueError};
use crate::lang::{
    common::notation::{mixfix::Mixfix, mixop::Mixop},
    data::shape::{Shape, ShapeArena, ShapeKind},
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
        shapes
            .fill(self.shape, self.args.iter().copied())
            .expect("a case has one argument per shape position")
    }

    /// Expands the notation with unfilled argument positions.
    pub fn to_mixop(&self, shapes: &ShapeArena) -> Mixop {
        shapes
            .fill(self.shape, std::iter::repeat_n((), self.args.len()))
            .expect("a case has one argument per shape position")
    }

    // - Printing

    /// Writes the filled notation as `Mixfix::print_with` does.
    pub fn print_with(
        &self,
        shapes: &ShapeArena,
        printer: &mut Printer<'_>,
        print_arg: impl FnMut(&Value, &mut Printer<'_>) -> fmt::Result,
    ) -> fmt::Result {
        self.to_mixfix(shapes).print_with(printer, print_arg)
    }

    // - Syntax comparison

    /// Orders two cases as their filled notations would order.
    ///
    /// Each case reads its shape in its own `ShapeArena`;
    /// arguments compare with `compare_arg` at their positions.
    pub(super) fn cmp_by(
        &self,
        shapes: &ShapeArena,
        value_case_other: &Self,
        shapes_other: &ShapeArena,
        mut compare_arg: impl FnMut(&Value, &Value) -> Ordering,
    ) -> Ordering {
        ShapeCmp {
            shapes_l: shapes,
            shapes_r: shapes_other,
            args_l: self.args.iter(),
            args_r: value_case_other.args.iter(),
            compare_arg: &mut compare_arg,
        }
        .cmp_shape(self.shape, value_case_other.shape)
    }
}

// = Shape comparison

/// Walks two shapes in step, consuming arguments in notation order.
struct ShapeCmp<'a, F> {
    shapes_l: &'a ShapeArena,
    shapes_r: &'a ShapeArena,
    args_l: slice::Iter<'a, Value>,
    args_r: slice::Iter<'a, Value>,
    compare_arg: &'a mut F,
}

impl<F: FnMut(&Value, &Value) -> Ordering> ShapeCmp<'_, F> {
    /// Compares one pair of nodes as `Mixfix::cmp_by` does.
    ///
    /// Both walks visit equal prefixes, so they reach positions in step.
    fn cmp_shape(&mut self, shape_l: Shape, shape_r: Shape) -> Ordering {
        let (shapes_l, shapes_r) = (self.shapes_l, self.shapes_r);
        let kind_l = shapes_l.kind(shape_l);
        let kind_r = shapes_r.kind(shape_r);
        match (kind_l, kind_r) {
            // Arguments compare at their position
            (ShapeKind::Arg, ShapeKind::Arg) => {
                let value_l = self.args_l.next().expect("a case fills every position");
                let value_r = self.args_r.next().expect("a case fills every position");
                (self.compare_arg)(value_l, value_r)
            }
            // Atoms compare by name, ignoring spans
            (ShapeKind::Atom(atom_l), ShapeKind::Atom(atom_r)) => atom_l.node.cmp(&atom_r.node),
            // Opening atom, inner shape, then closing atom
            (
                ShapeKind::Brack(atom_l_l, shape_inner_l, atom_l_r),
                ShapeKind::Brack(atom_r_l, shape_inner_r, atom_r_r),
            ) => atom_l_l
                .node
                .cmp(&atom_r_l.node)
                .then_with(|| self.cmp_shape(*shape_inner_l, *shape_inner_r))
                .then_with(|| atom_l_r.node.cmp(&atom_r_r.node)),
            // Left shape, operator, then right shape
            (
                ShapeKind::Infix(shape_l_l, atom_l, shape_l_r),
                ShapeKind::Infix(shape_r_l, atom_r, shape_r_r),
            ) => self
                .cmp_shape(*shape_l_l, *shape_r_l)
                .then_with(|| atom_l.node.cmp(&atom_r.node))
                .then_with(|| self.cmp_shape(*shape_l_r, *shape_r_r)),
            // Common prefix first, then length
            (ShapeKind::Seq(shapes_seq_l), ShapeKind::Seq(shapes_seq_r)) => {
                for (shape_l, shape_r) in shapes_seq_l.iter().zip(shapes_seq_r) {
                    let order = self.cmp_shape(*shape_l, *shape_r);
                    if order != Ordering::Equal {
                        return order;
                    }
                }
                shapes_seq_l.len().cmp(&shapes_seq_r.len())
            }
            // Different node kinds order by variant
            _ => kind_l.tag().cmp(&kind_r.tag()),
        }
    }
}
