//! Tree representation of notation: children kept in place
//!
//! `MixopTree` boxes lone children and keeps sequence elements inline,
//! owning its whole form.
//! Equality, ordering, and hashing read atom names, never atom spans,
//! and printing writes `%` at each argument position.

use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
};

use crate::lang::traits::print::{Print, Printer};

use super::{AtomPhrase, walk};

/// An owned notation with an argument hole at each position.
#[derive(Clone, Debug)]
pub enum MixopTree {
    Arg,
    Atom(AtomPhrase),
    Brack(AtomPhrase, Box<MixopTree>, AtomPhrase),
    Infix(Box<MixopTree>, AtomPhrase, Box<MixopTree>),
    Seq(Vec<MixopTree>),
}

impl MixopTree {
    /// Counts argument positions in notation order.
    pub fn arity(&self) -> usize {
        walk::arity_tree(self)
    }
    /// Orders the variants for comparison across forms.
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

// = Equality, ordering, and hashing

impl PartialEq for MixopTree {
    fn eq(&self, node_other: &Self) -> bool {
        self.cmp(node_other).is_eq()
    }
}

impl Eq for MixopTree {}

impl Ord for MixopTree {
    fn cmp(&self, node_other: &Self) -> Ordering {
        walk::cmp_trees_by(self, node_other, |_| Ordering::Equal)
    }
}

impl PartialOrd for MixopTree {
    fn partial_cmp(&self, node_other: &Self) -> Option<Ordering> {
        Some(self.cmp(node_other))
    }
}

impl Hash for MixopTree {
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

impl Print for MixopTree {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        walk::print_tree_with(self, printer, |_, printer| printer.write("%"))
    }
}
