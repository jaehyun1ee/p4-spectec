//! Append-only storage for notation mixops
//!
//! A mixop handle is valid only in the arena that issued it.
//! `intern` stores a tree node by node, children first,
//! and records each new mixop's number of argument positions;
//! `intern_shared` remembers a shared tree by its address,
//! so a mixop shared by many expressions is walked once.

use std::{collections::HashMap, rc::Rc};

use foldhash::fast::RandomState;

use crate::lang::data::intern::{CanonId, CanonInterner};

use super::{error::MixopError, flat, tree};

// = Arena storage

/// A shared tree with its mixop, kept so the tree's address is not reused.
type SharedMixop = (Rc<tree::Mixop>, flat::Mixop);

/// Storage for notation mixops, shared by every value built from them.
#[derive(Debug, Default)]
pub struct MixopArena {
    /// Mixops, with canonical identities that ignore atom spans.
    mixops: CanonInterner<flat::MixopKind>,
    /// Argument positions of each mixop, by handle index.
    arities: Vec<u32>,
    /// Handles of shared trees, by address.
    shared: HashMap<*const tree::Mixop, SharedMixop, RandomState>,
}

impl MixopArena {
    // - Construction

    /// An empty arena.
    pub fn new() -> Self {
        Self::default()
    }

    // - Interning

    /// Interns a tree node by node, children first, copying atoms.
    pub fn intern(&mut self, mixop: &tree::Mixop) -> Result<flat::Mixop, MixopError> {
        // Children get their canonical identities before the parent is hashed
        let kind = match mixop {
            tree::Mixop::Arg => flat::MixopKind::Arg,
            tree::Mixop::Atom(atom) => flat::MixopKind::Atom(atom.clone()),
            tree::Mixop::Brack(atom_l, mixop_inner, atom_r) => {
                let mixop_id_inner = self.intern(mixop_inner)?;
                flat::MixopKind::Brack(atom_l.clone(), mixop_id_inner, atom_r.clone())
            }
            tree::Mixop::Infix(mixop_l, atom, mixop_r) => {
                let mixop_id_l = self.intern(mixop_l)?;
                let mixop_id_r = self.intern(mixop_r)?;
                flat::MixopKind::Infix(mixop_id_l, atom.clone(), mixop_id_r)
            }
            tree::Mixop::Seq(mixops) => flat::MixopKind::Seq(
                mixops
                    .iter()
                    .map(|mixop| self.intern(mixop))
                    .collect::<Result<_, _>>()?,
            ),
        };
        let mixop_id = self.mixops.intern(kind, &())?;
        // A new mixop sums its children's positions, which are recorded
        if mixop_id.index() as usize == self.arities.len() {
            let arity = self.arity_kind(self.kind(mixop_id));
            self.arities.push(u32::try_from(arity)?);
        }
        Ok(mixop_id)
    }

    /// Interns a shared tree, walking it only the first time.
    pub fn intern_shared(&mut self, mixop: &Rc<tree::Mixop>) -> Result<flat::Mixop, MixopError> {
        // Seen before: the same allocation has the same mixop
        if let Some((_, mixop_id)) = self.shared.get(&Rc::as_ptr(mixop)) {
            return Ok(*mixop_id);
        }
        // First sight: intern and keep the tree alive with its mixop
        let mixop_id = self.intern(mixop)?;
        self.shared
            .insert(Rc::as_ptr(mixop), (Rc::clone(mixop), mixop_id));
        Ok(mixop_id)
    }

    /// Counts a node's positions from its children's recorded counts.
    fn arity_kind(&self, kind: &flat::MixopKind) -> usize {
        match kind {
            flat::MixopKind::Arg => 1,
            flat::MixopKind::Atom(_) => 0,
            flat::MixopKind::Brack(_, mixop_id, _) => self.arity(*mixop_id),
            flat::MixopKind::Infix(mixop_id_l, _, mixop_id_r) => {
                self.arity(*mixop_id_l) + self.arity(*mixop_id_r)
            }
            flat::MixopKind::Seq(mixops) => {
                mixops.iter().map(|mixop_id| self.arity(*mixop_id)).sum()
            }
        }
    }

    // - Lookup

    /// The node behind a mixop handle.
    pub fn kind(&self, mixop_id: flat::Mixop) -> &flat::MixopKind {
        self.mixops.get(mixop_id)
    }

    /// The number of argument positions of a mixop.
    pub fn arity(&self, mixop_id: flat::Mixop) -> usize {
        self.arities[mixop_id.index() as usize] as usize
    }

    /// The canonical identity of a mixop, ignoring atom spans.
    pub fn canon_id(&self, mixop_id: flat::Mixop) -> CanonId<flat::MixopKind> {
        self.mixops.canon_id(mixop_id)
    }

    /// Whether two mixops have the same structure and atom names.
    pub fn canon_eq(&self, mixop_id_l: flat::Mixop, mixop_id_r: flat::Mixop) -> bool {
        self.canon_id(mixop_id_l) == self.canon_id(mixop_id_r)
    }
}
