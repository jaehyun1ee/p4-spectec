//! Append-only storage for notation shapes
//!
//! A shape handle is valid only in the arena that issued it.
//! `intern` stores a tree node by node, children first,
//! and records each new shape's number of argument positions;
//! `intern_shared` remembers a shared tree by its address,
//! so a mixop shared by many expressions is walked once.
//! Comparison with trees and expansion go through `walk`.

use std::{collections::HashMap, rc::Rc};

use foldhash::fast::RandomState;

use crate::lang::data::intern::{CanonId, CanonInterner};

use super::{
    error::ShapeError,
    handle::{Shape, ShapeKind},
    node::Node,
    tree::Tree,
    walk,
};

// = Arena storage

/// A shared tree with its shape, kept so the tree's address is not reused.
type SharedShape = (Rc<Node<Tree>>, Shape);

/// Storage for notation shapes, shared by every value built from them.
#[derive(Debug, Default)]
pub struct ShapeArena {
    /// Shapes, with canonical identities that ignore atom spans.
    shapes: CanonInterner<ShapeKind>,
    /// Argument positions of each shape, by handle index.
    arities: Vec<u32>,
    /// Shapes of shared trees, by address.
    shared: HashMap<*const Node<Tree>, SharedShape, RandomState>,
}

impl ShapeArena {
    // - Construction

    /// An empty arena.
    pub fn new() -> Self {
        Self::default()
    }

    // - Interning

    /// Interns a tree node by node, children first, copying atoms.
    pub fn intern(&mut self, mixop: &Node<Tree>) -> Result<Shape, ShapeError> {
        // Children get their canonical identities before the parent is hashed
        let kind = match mixop {
            Node::Arg => ShapeKind::Arg,
            Node::Atom(atom) => ShapeKind::Atom(atom.clone()),
            Node::Brack(atom_l, mixop_inner, atom_r) => {
                let shape_inner = self.intern(mixop_inner)?;
                ShapeKind::Brack(atom_l.clone(), shape_inner, atom_r.clone())
            }
            Node::Infix(mixop_l, atom, mixop_r) => {
                let shape_l = self.intern(mixop_l)?;
                let shape_r = self.intern(mixop_r)?;
                ShapeKind::Infix(shape_l, atom.clone(), shape_r)
            }
            Node::Seq(mixops) => ShapeKind::Seq(
                mixops
                    .iter()
                    .map(|mixop| self.intern(mixop))
                    .collect::<Result<_, _>>()?,
            ),
        };
        let shape = self.shapes.intern(kind, &())?;
        // A new shape sums its children's positions, which are recorded
        if shape.index() as usize == self.arities.len() {
            let arity = self.arity_kind(self.kind(shape));
            self.arities.push(u32::try_from(arity)?);
        }
        Ok(shape)
    }

    /// Interns a shared tree, walking it only the first time.
    pub fn intern_shared(&mut self, mixop: &Rc<Node<Tree>>) -> Result<Shape, ShapeError> {
        // Seen before: the same allocation has the same shape
        if let Some((_, shape)) = self.shared.get(&Rc::as_ptr(mixop)) {
            return Ok(*shape);
        }
        // First sight: intern and keep the tree alive with its shape
        let shape = self.intern(mixop)?;
        self.shared
            .insert(Rc::as_ptr(mixop), (Rc::clone(mixop), shape));
        Ok(shape)
    }

    /// Counts a node's positions from its children's recorded counts.
    fn arity_kind(&self, kind: &ShapeKind) -> usize {
        match kind {
            ShapeKind::Arg => 1,
            ShapeKind::Atom(_) => 0,
            ShapeKind::Brack(_, shape, _) => self.arity(*shape),
            ShapeKind::Infix(shape_l, _, shape_r) => self.arity(*shape_l) + self.arity(*shape_r),
            ShapeKind::Seq(shapes) => shapes.iter().map(|shape| self.arity(*shape)).sum(),
        }
    }

    // - Lookup

    /// The node behind a shape handle.
    pub fn kind(&self, shape: Shape) -> &ShapeKind {
        self.shapes.get(shape)
    }

    /// The number of argument positions of a shape.
    pub fn arity(&self, shape: Shape) -> usize {
        self.arities[shape.index() as usize] as usize
    }

    /// The canonical identity of a shape, ignoring atom spans.
    pub fn canon_id(&self, shape: Shape) -> CanonId<ShapeKind> {
        self.shapes.canon_id(shape)
    }

    /// Whether two shapes have the same structure and atom names.
    pub fn canon_eq(&self, shape_l: Shape, shape_r: Shape) -> bool {
        self.canon_id(shape_l) == self.canon_id(shape_r)
    }

    // - Trees

    /// Whether a shape has a tree's structure and atom names.
    pub fn eq_mixop(&self, shape: Shape, mixop: &Node<Tree>) -> bool {
        walk::eq(self, self.kind(shape), &(), mixop)
    }

    /// Expands a shape into a tree, copying atoms with their spans.
    pub fn to_mixop(&self, shape: Shape) -> Node<Tree> {
        walk::to_tree(self, self.kind(shape))
    }
}
