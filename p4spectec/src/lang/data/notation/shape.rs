//! Handle representation of notation: `ShapeKind` and `Shape`
//!
//! `Handle` holds a node's children as handles into a `ShapeArena`,
//! so `Brack(atom_l, shape, atom_r)` refers to its inner shape by handle
//! and equal subtrees are stored once.
//! Exact equality and hashing include atom spans and compare children
//! by handle; `CanonEq` and `CanonHash` read atom names
//! and children's canonical ids, so they ignore spans.

use std::{
    fmt,
    hash::{Hash, Hasher},
};

use crate::lang::data::intern::{CanonEq, CanonHash, CanonInterner, Interned};

use super::{
    arena::ShapeArena,
    node::{Expand, Node, Repr},
};

// = Representation

/// Children held as handles into a `ShapeArena`.
#[derive(Clone, Copy, Debug)]
pub struct Handle;

impl Repr<()> for Handle {
    type Child = Shape;
    type Children = Vec<Shape>;
}

impl Expand<()> for Handle {
    type Ctx = ShapeArena;

    fn child<'a>(shapes: &'a ShapeArena, shape: &'a Shape) -> &'a ShapeKind
    where
        (): 'a,
    {
        shapes.kind(*shape)
    }

    fn children<'a>(
        shapes: &'a ShapeArena,
        shapes_seq: &'a Vec<Shape>,
    ) -> impl ExactSizeIterator<Item = &'a ShapeKind> + 'a
    where
        (): 'a,
    {
        shapes_seq.iter().map(move |shape| shapes.kind(*shape))
    }
}

// = Shapes

/// A notation node whose children are handles; arguments are holes.
pub type ShapeKind = Node<(), Handle>;

/// A notation handle valid only in the `ShapeArena` that issued it.
pub type Shape = Interned<ShapeKind>;

// = Exact equality and hashing

// Exact identity: atoms with their spans, children by handle number

impl PartialEq for ShapeKind {
    fn eq(&self, kind_other: &Self) -> bool {
        match (self, kind_other) {
            (Self::Arg(()), Self::Arg(())) => true,
            (Self::Atom(atom_l), Self::Atom(atom_r)) => atom_l == atom_r,
            (
                Self::Brack(atom_l_l, shape_l, atom_l_r),
                Self::Brack(atom_r_l, shape_r, atom_r_r),
            ) => atom_l_l == atom_r_l && shape_l == shape_r && atom_l_r == atom_r_r,
            (
                Self::Infix(shape_l_l, atom_l, shape_l_r),
                Self::Infix(shape_r_l, atom_r, shape_r_r),
            ) => shape_l_l == shape_r_l && atom_l == atom_r && shape_l_r == shape_r_r,
            (Self::Seq(shapes_l), Self::Seq(shapes_r)) => shapes_l == shapes_r,
            _ => false,
        }
    }
}

impl Eq for ShapeKind {}

impl Hash for ShapeKind {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        self.tag().hash(hasher);
        match self {
            Self::Arg(()) => {}
            Self::Atom(atom) => atom.hash(hasher),
            Self::Brack(atom_l, shape, atom_r) => {
                atom_l.hash(hasher);
                shape.hash(hasher);
                atom_r.hash(hasher);
            }
            Self::Infix(shape_l, atom, shape_r) => {
                shape_l.hash(hasher);
                atom.hash(hasher);
                shape_r.hash(hasher);
            }
            Self::Seq(shapes) => shapes.hash(hasher),
        }
    }
}

// - Debugging

// The same text a derive prints: variant names and fields, no type name
impl fmt::Debug for ShapeKind {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Arg(()) => fmt.debug_tuple("Arg").field(&()).finish(),
            Self::Atom(atom) => fmt.debug_tuple("Atom").field(atom).finish(),
            Self::Brack(atom_l, shape, atom_r) => fmt
                .debug_tuple("Brack")
                .field(atom_l)
                .field(shape)
                .field(atom_r)
                .finish(),
            Self::Infix(shape_l, atom, shape_r) => fmt
                .debug_tuple("Infix")
                .field(shape_l)
                .field(atom)
                .field(shape_r)
                .finish(),
            Self::Seq(shapes) => fmt.debug_tuple("Seq").field(shapes).finish(),
        }
    }
}

// = Canonical equality and hashing

// Canonical identity: atom names and children's canonical ids, so spans
// are ignored as a tree ignores them

impl CanonEq for ShapeKind {
    fn canon_eq(&self, interner: &CanonInterner<Self>, _: &(), kind_r: &Self) -> bool {
        // Children compare by canonical id, computed when they were interned
        let eq_shape = |shape_l: &Shape, shape_r: &Shape| {
            interner.canon_id(*shape_l) == interner.canon_id(*shape_r)
        };
        match (self, kind_r) {
            (Self::Arg(()), Self::Arg(())) => true,
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
            Self::Arg(()) => {}
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
