//! Tree representation of notation: children kept in place
//!
//! `Mixop` boxes lone children and keeps sequence elements inline,
//! owning its whole form.
//! Equality, ordering, and hashing read atom names, never atom spans,
//! and printing writes `%` at each argument position.

use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    rc::Rc,
};

use crate::lang::{
    common::ds::set::IdSet,
    traits::{
        eq::SyntaxEq,
        free::FreeIds,
        print::{Print, Printer},
    },
};

use super::{AtomPhrase, MixopArena, MixopError, MixopId, mixop::MixopMatch, walk};
use serde::{Deserialize, Serialize};

/// An owned notation with an argument hole at each position.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Mixop {
    Arg,
    Atom(AtomPhrase),
    Brack(AtomPhrase, Box<Mixop>, AtomPhrase),
    Infix(Box<Mixop>, AtomPhrase, Box<Mixop>),
    Seq(Vec<Mixop>),
}

impl Mixop {
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

// = Matching

impl MixopMatch for Rc<Mixop> {
    fn matches_mixop(&self, arena_mixop: &MixopArena, mixop_id: MixopId) -> bool {
        arena_mixop.matches_tree(mixop_id, self)
    }
}

// = Equality, ordering, and hashing

impl PartialEq for Mixop {
    fn eq(&self, node_other: &Self) -> bool {
        self.cmp(node_other).is_eq()
    }
}

impl Eq for Mixop {}

impl Ord for Mixop {
    fn cmp(&self, node_other: &Self) -> Ordering {
        walk::cmp_trees_by(self, node_other, |_| Ordering::Equal)
    }
}

impl PartialOrd for Mixop {
    fn partial_cmp(&self, node_other: &Self) -> Option<Ordering> {
        Some(self.cmp(node_other))
    }
}

impl Hash for Mixop {
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

// = Syntax operations

impl SyntaxEq for Mixop {
    fn syntax_eq(&self, mixop_other: &Self) -> bool {
        self == mixop_other
    }
}

impl FreeIds for Mixop {
    fn free_ids(&self) -> IdSet {
        IdSet::new()
    }
}

// = Printing

impl Print for Mixop {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        walk::print_tree_with(self, printer, |_, printer| printer.write("%"))
    }
}

/// Expands a handle into an owned notation, preserving atom spans.
pub fn from_flat(arena_mixop: &MixopArena, mixop_id: MixopId) -> Mixop {
    arena_mixop.to_tree(mixop_id)
}

/// Interns an owned notation in the target arena.
pub fn into_flat(arena_mixop: &mut MixopArena, mixop_tree: Mixop) -> Result<MixopId, MixopError> {
    arena_mixop.intern(&mixop_tree)
}
