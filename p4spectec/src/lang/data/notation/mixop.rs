//! Mixop matching and cached parsing
//!
//! `MixopMatch` lets evaluation match source or prepared syntax
//! against a stored value case;
//! `shape` parses a mixop from its text once and caches it.

use std::{cell::RefCell, collections::HashMap, fmt, rc::Rc};

use crate::frontend;

use super::{arena::MixopArena, flat::MixopId, tree::Mixop};

// == Mixops as a stage holds them

/// Matches a stored value case against source or prepared notation.
pub trait MixopMatch: Clone + fmt::Debug + PartialEq {
    /// Compares notation structure and atom names, ignoring spans.
    fn matches_mixop(&self, arena_mixop: &MixopArena, mixop_id: MixopId) -> bool;
}

// = Mixop parsing

thread_local! {
    /// Parsed mixops by their source text.
    static SHAPE_CACHE: RefCell<HashMap<Rc<str>, Rc<Mixop>>> = RefCell::new(HashMap::new());
}

/// Parses a mixop from its text, reusing an earlier parse of the same text.
pub(crate) fn shape(shape_text: &str) -> Rc<Mixop> {
    SHAPE_CACHE.with(|cache| {
        // Cached: share it
        if let Some(mixop) = cache.borrow().get(shape_text).cloned() {
            return mixop;
        }

        // First use: parse and remember
        let mixop = frontend::parse::parse_mixop(shape_text)
            .expect("value constructor contains a valid SpecTec mixop");
        let mixop = Rc::new(mixop);
        cache
            .borrow_mut()
            .insert(Rc::from(shape_text), Rc::clone(&mixop));
        mixop
    })
}
