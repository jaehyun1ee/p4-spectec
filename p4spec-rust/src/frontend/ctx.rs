//! Parser state shared by grammar actions and contextual tokenization
//!
//! `Bindings` owns variable names that survive across related source
//! files. `Context::with_bindings` creates fresh per-source scopes,
//! parser modes, and interned positions around those bindings. Grammar actions
//! pair `enter_scope` with `exit_scope` and `enter_exp` or `enter_arith` with
//! `exit_mode`; the token adapter reads `in_arith` while classifying `*`.
//! `location` turns a [`Position`] into a compact [`Location`]
//! that LALRPOP can copy and later resolve through `position` or `span`.
//!
//! # Example
//!
//! ```text
//! Bindings
//! ├── Context(file_a): scopes_a, modes_a, positions_a
//! └── Context(file_b): scopes_b, modes_b, positions_b
//! ```

use std::{cell::RefCell, collections::BTreeSet, rc::Rc};

use crate::lang::{
    common::source::{Position, Span},
    xl,
};

/// A compact source location for LALRPOP
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Location(usize);

/// Variable bindings preserved across related SpecTec source files
#[derive(Default)]
pub(crate) struct Bindings {
    variables: RefCell<BTreeSet<String>>,
}

/// Parser mode for contextual tokenization
#[derive(Clone, Copy)]
enum Mode {
    Exp,
    Arith,
}

/// Per-source state shared by parser actions and contextual lexing
pub(crate) struct Context {
    bindings: Rc<Bindings>,
    scopes: RefCell<Vec<Vec<String>>>,
    positions: RefCell<Vec<Position>>,
    modes: RefCell<Vec<Mode>>,
}

impl Context {
    // - Construction

    pub(crate) fn with_bindings(bindings: Rc<Bindings>) -> Self {
        Self {
            bindings,
            scopes: RefCell::default(),
            positions: RefCell::default(),
            modes: RefCell::default(),
        }
    }

    // - Source locations

    pub(crate) fn location(&self, position: Position) -> Location {
        let mut positions = self.positions.borrow_mut();
        let location = Location(positions.len());
        positions.push(position);
        location
    }

    pub(crate) fn position(&self, location: Location) -> Position {
        self.positions.borrow()[location.0].clone()
    }

    pub(crate) fn span(&self, left: Location, right: Location) -> Span {
        Span::new(self.position(left), self.position(right))
    }

    // - Variable scopes

    pub(crate) fn enter_scope(&self) {
        self.scopes.borrow_mut().push(Vec::new());
    }

    pub(crate) fn exit_scope(&self) {
        let identifiers = self
            .scopes
            .borrow_mut()
            .pop()
            .expect("parser scope actions are balanced");
        let mut variables = self.bindings.variables.borrow_mut();
        for identifier in identifiers {
            variables.remove(&identifier);
        }
    }

    pub(crate) fn add_id(&self, identifier: &str) {
        let identifier = identifier.to_owned();
        if self
            .bindings
            .variables
            .borrow_mut()
            .insert(identifier.clone())
            && let Some(scope) = self.scopes.borrow_mut().last_mut()
        {
            scope.push(identifier);
        }
    }

    pub(crate) fn find_id(&self, identifier: &str) -> bool {
        self.bindings
            .variables
            .borrow()
            .contains(xl::var::strip_var_suffix_name(identifier))
    }

    // - Parser modes

    pub(crate) fn enter_exp(&self) {
        self.modes.borrow_mut().push(Mode::Exp);
    }

    pub(crate) fn enter_arith(&self) {
        self.modes.borrow_mut().push(Mode::Arith);
    }

    pub(crate) fn exit_mode(&self) {
        self.modes
            .borrow_mut()
            .pop()
            .expect("parser mode actions are balanced");
    }

    pub(crate) fn in_arith(&self) -> bool {
        matches!(self.modes.borrow().last(), Some(Mode::Arith))
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::with_bindings(Rc::new(Bindings::default()))
    }
}
