//! Mixops, notation forms without arguments
//!
//! A `Mixop` is a `Node<Tree>`:
//! the atoms of a notation form and where its arguments go.
//! `MixopRepr` is how a syntax stage holds one, so code generic
//! over the stage prints notations without knowing which;
//! `shape` parses a mixop from its text once and caches it.

use std::{cell::RefCell, collections::HashMap, fmt, rc::Rc};

use crate::lang::{
    common::ds::set::IdSet,
    traits::{eq::SyntaxEq, free::FreeIds, print::Printer},
};

use crate::frontend;

use super::{arena::ShapeArena, flat::Shape, node::Node, tree::Tree, walk};

/// A notation form with argument positions and no arguments.
pub type Mixop = Node<Tree>;

// == Mixops as a stage holds them

/// A mixop as a syntax stage holds it: a shared tree or an interned shape.
pub trait MixopRepr: Clone + fmt::Debug + PartialEq {
    /// Writes the form, with `print_arg` writing the argument at each position.
    fn print_with(
        &self,
        printer: &mut Printer<'_>,
        print_arg: impl FnMut(usize, &mut Printer<'_>) -> fmt::Result,
    ) -> fmt::Result;

    /// Whether a shape has this mixop's structure and atom names.
    fn matches_shape(&self, arena_shape: &ShapeArena, shape: Shape) -> bool;
}

impl MixopRepr for Rc<Mixop> {
    fn print_with(
        &self,
        printer: &mut Printer<'_>,
        print_arg: impl FnMut(usize, &mut Printer<'_>) -> fmt::Result,
    ) -> fmt::Result {
        walk::print_with(&(), self.as_ref(), printer, print_arg)
    }

    fn matches_shape(&self, arena_shape: &ShapeArena, shape: Shape) -> bool {
        arena_shape.eq_mixop(shape, self)
    }
}

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
