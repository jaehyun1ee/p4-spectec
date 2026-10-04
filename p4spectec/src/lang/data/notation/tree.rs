//! Tree representation of notation: children kept in place
//!
//! `Tree` boxes a lone child and keeps sequence elements inline,
//! so a `Node<Tree>` owns its whole form.
//! Equality, ordering, and hashing read atom names, never atom spans,
//! and printing writes `%` at each argument position.

use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
};

use crate::lang::traits::print::{Print, Printer};

use super::{
    node::{Node, Repr},
    walk,
};

// = Representation

/// Children kept in place: lone children boxed, sequence elements inline.
#[derive(Clone, Copy, Debug)]
pub struct Tree;

impl Repr for Tree {
    type Child = Box<Node<Tree>>;
    type Elem = Node<Tree>;
    type ShapeArena = ();

    fn node<'a>((): &'a (), elem: &'a Node<Tree>) -> &'a Node<Tree> {
        elem
    }
}

// = Cloning and debugging

impl Clone for Node<Tree> {
    fn clone(&self) -> Self {
        match self {
            Self::Arg => Self::Arg,
            Self::Atom(atom) => Self::Atom(atom.clone()),
            Self::Brack(atom_l, child, atom_r) => {
                Self::Brack(atom_l.clone(), child.clone(), atom_r.clone())
            }
            Self::Infix(child_l, atom, child_r) => {
                Self::Infix(child_l.clone(), atom.clone(), child_r.clone())
            }
            Self::Seq(elems) => Self::Seq(elems.clone()),
        }
    }
}

// The same text a derive prints: variant names and fields, no type name
impl fmt::Debug for Node<Tree> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Arg => fmt.write_str("Arg"),
            Self::Atom(atom) => fmt.debug_tuple("Atom").field(atom).finish(),
            Self::Brack(atom_l, child, atom_r) => fmt
                .debug_tuple("Brack")
                .field(atom_l)
                .field(child)
                .field(atom_r)
                .finish(),
            Self::Infix(child_l, atom, child_r) => fmt
                .debug_tuple("Infix")
                .field(child_l)
                .field(atom)
                .field(child_r)
                .finish(),
            Self::Seq(elems) => fmt.debug_tuple("Seq").field(elems).finish(),
        }
    }
}

// = Equality, ordering, and hashing

impl PartialEq for Node<Tree> {
    fn eq(&self, node_other: &Self) -> bool {
        walk::eq(&(), self, &(), node_other)
    }
}

impl Eq for Node<Tree> {}

impl Ord for Node<Tree> {
    fn cmp(&self, node_other: &Self) -> Ordering {
        walk::cmp_by(&(), self, &(), node_other, |_| Ordering::Equal)
    }
}

impl PartialOrd for Node<Tree> {
    fn partial_cmp(&self, node_other: &Self) -> Option<Ordering> {
        Some(self.cmp(node_other))
    }
}

impl Hash for Node<Tree> {
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

// = Printing

impl Print for Node<Tree> {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        walk::print_with(&(), self, printer, |_, printer| printer.write("%"))
    }
}
