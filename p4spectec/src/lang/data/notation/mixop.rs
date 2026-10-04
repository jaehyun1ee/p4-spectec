//! Mixops, notation forms without arguments
//!
//! A `Mixop` is a `Node<Tree>`:
//! the atoms of a notation form and where its arguments go.
//! `shape` parses a mixop from its text once and caches it.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::lang::{
    common::ds::set::IdSet,
    traits::{eq::SyntaxEq, free::FreeIds},
};

use crate::frontend;

use super::{node::Node, tree::Tree};

/// A notation form with argument positions and no arguments.
pub type Mixop = Node<Tree>;

// == Syntax operations

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

// = Shape parsing

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
