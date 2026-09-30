//! Notation nodes whose children are shape handles
//!
//! `Brack(atom_l, shape, atom_r)` refers to its inner shape by handle,
//! so equal subtrees are stored once and compared by identity.
//! Exact equality and hashing include atom spans;
//! `CanonEq` and `CanonHash` read atom names and children's canonical ids.

use std::hash::{Hash, Hasher};

use crate::lang::{
    common::notation::mixfix::AtomPhrase,
    data::intern::{CanonEq, CanonHash, CanonInterner, Interned},
};

// = Shapes

/// A notation handle valid only in the `ShapeArena` that issued it.
pub type Shape = Interned<ShapeKind>;

/// A notation node: atoms and argument holes, with children as handles.
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum ShapeKind {
    /// Argument position.
    Arg,
    /// Literal atom.
    Atom(AtomPhrase),
    /// Bracketed shape.
    Brack(AtomPhrase, Shape, AtomPhrase),
    /// Infix shape.
    Infix(Shape, AtomPhrase, Shape),
    /// Sequence of shapes.
    Seq(Vec<Shape>),
}

impl ShapeKind {
    /// Orders the variants as `Mixfix` comparison does.
    pub(crate) fn tag(&self) -> u8 {
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

impl CanonEq for ShapeKind {
    fn canon_eq(&self, interner: &CanonInterner<Self>, _: &(), kind_r: &Self) -> bool {
        // Children compare by canonical id, computed when they were interned
        let eq_shape = |shape_l: &Shape, shape_r: &Shape| {
            interner.canon_id(*shape_l) == interner.canon_id(*shape_r)
        };
        match (self, kind_r) {
            (Self::Arg, Self::Arg) => true,
            (Self::Atom(atom_l), Self::Atom(atom_r)) => atom_l.node == atom_r.node,
            (
                Self::Brack(atom_l_l, shape_l, atom_l_r),
                Self::Brack(atom_r_l, shape_r, atom_r_r),
            ) => {
                atom_l_l.node == atom_r_l.node
                    && eq_shape(shape_l, shape_r)
                    && atom_l_r.node == atom_r_r.node
            }
            (
                Self::Infix(shape_l_l, atom_l, shape_l_r),
                Self::Infix(shape_r_l, atom_r, shape_r_r),
            ) => {
                eq_shape(shape_l_l, shape_r_l)
                    && atom_l.node == atom_r.node
                    && eq_shape(shape_l_r, shape_r_r)
            }
            (Self::Seq(shapes_l), Self::Seq(shapes_r)) => {
                shapes_l.len() == shapes_r.len()
                    && shapes_l
                        .iter()
                        .zip(shapes_r)
                        .all(|(shape_l, shape_r)| eq_shape(shape_l, shape_r))
            }
            _ => false,
        }
    }
}

impl CanonHash for ShapeKind {
    fn canon_hash<H: Hasher>(&self, interner: &CanonInterner<Self>, _: &(), hasher: &mut H) {
        self.tag().hash(hasher);
        match self {
            Self::Arg => {}
            Self::Atom(atom) => atom.node.hash(hasher),
            Self::Brack(atom_l, shape, atom_r) => {
                atom_l.node.hash(hasher);
                interner.canon_id(*shape).hash(hasher);
                atom_r.node.hash(hasher);
            }
            Self::Infix(shape_l, atom, shape_r) => {
                interner.canon_id(*shape_l).hash(hasher);
                atom.node.hash(hasher);
                interner.canon_id(*shape_r).hash(hasher);
            }
            Self::Seq(shapes) => {
                shapes.len().hash(hasher);
                for shape in shapes {
                    interner.canon_id(*shape).hash(hasher);
                }
            }
        }
    }
}
