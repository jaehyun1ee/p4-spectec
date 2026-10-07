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
        mixop.to_flat(self)
    }

    /// Interns a node whose children already belong to this arena.
    pub(super) fn intern_kind(&mut self, kind: flat::MixopKind) -> Result<flat::Mixop, MixopError> {
        // Child canonical identities are available when the parent is hashed
        let mixop = self.mixops.intern(kind, &())?;
        // A new mixop sums its children's positions, which are recorded
        if mixop.index() as usize == self.arities.len() {
            let arity = self.kind(mixop).arity(self);
            self.arities.push(u32::try_from(arity)?);
        }
        Ok(mixop)
    }

    /// Interns a shared tree, walking it only the first time.
    pub fn intern_shared(
        &mut self,
        mixop_tree: &Rc<tree::Mixop>,
    ) -> Result<flat::Mixop, MixopError> {
        // Seen before: the same allocation has the same mixop
        if let Some((_, mixop)) = self.shared.get(&Rc::as_ptr(mixop_tree)) {
            return Ok(*mixop);
        }
        // First sight: intern and keep the tree alive with its mixop
        let mixop = self.intern(mixop_tree)?;
        self.shared
            .insert(Rc::as_ptr(mixop_tree), (Rc::clone(mixop_tree), mixop));
        Ok(mixop)
    }

    // - Lookup

    /// The node behind a mixop handle.
    pub fn kind(&self, mixop: flat::Mixop) -> &flat::MixopKind {
        self.mixops.get(mixop)
    }

    /// The number of argument positions of a mixop.
    pub fn arity(&self, mixop: flat::Mixop) -> usize {
        self.arities[mixop.index() as usize] as usize
    }

    /// The canonical identity of a mixop, ignoring atom spans.
    pub fn canon_id(&self, mixop: flat::Mixop) -> CanonId<flat::MixopKind> {
        self.mixops.canon_id(mixop)
    }

    /// Whether two mixops have the same structure and atom names.
    pub fn canon_eq(&self, mixop_l: flat::Mixop, mixop_r: flat::Mixop) -> bool {
        self.canon_id(mixop_l) == self.canon_id(mixop_r)
    }
}
