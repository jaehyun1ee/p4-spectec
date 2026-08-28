//! Parser state shared by grammar actions and contextual tokenization

use std::{cell::RefCell, collections::BTreeSet, rc::Rc};

use crate::lang::{
    common::source::{Position, Span},
    xl,
};

/// Variable bindings preserved across related SpecTec source files
#[derive(Default)]
pub(crate) struct ParserBindings {
    variables: RefCell<BTreeSet<String>>,
}

/// Per-source state shared by parser actions and contextual lexing
pub(crate) struct ParserContext {
    bindings: Rc<ParserBindings>,
    scopes: RefCell<Vec<Vec<String>>>,
    positions: RefCell<Vec<Position>>,
    modes: RefCell<Vec<ParserMode>>,
}

#[derive(Clone, Copy)]
enum ParserMode {
    Exp,
    Arith,
}

impl Default for ParserContext {
    fn default() -> Self {
        Self::with_bindings(Rc::new(ParserBindings::default()))
    }
}

impl ParserContext {
    pub(crate) fn with_bindings(bindings: Rc<ParserBindings>) -> Self {
        Self {
            bindings,
            scopes: RefCell::default(),
            positions: RefCell::default(),
            modes: RefCell::default(),
        }
    }

    pub(crate) fn is_var(&self, identifier: &str) -> bool {
        self.bindings
            .variables
            .borrow()
            .contains(xl::var::strip_var_suffix_name(identifier))
    }

    pub(crate) fn bind(&self, identifier: &str) {
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

    pub(crate) fn intern_position(&self, position: Position) -> ParserLocation {
        let mut positions = self.positions.borrow_mut();
        let location = ParserLocation(positions.len());
        positions.push(position);
        location
    }

    pub(crate) fn position(&self, location: ParserLocation) -> Position {
        self.positions.borrow()[location.0].clone()
    }

    pub(crate) fn span(&self, left: ParserLocation, right: ParserLocation) -> Span {
        Span::new(self.position(left), self.position(right))
    }

    pub(crate) fn enter_exp(&self) {
        self.modes.borrow_mut().push(ParserMode::Exp);
    }

    pub(crate) fn enter_arith(&self) {
        self.modes.borrow_mut().push(ParserMode::Arith);
    }

    pub(crate) fn exit_mode(&self) {
        self.modes
            .borrow_mut()
            .pop()
            .expect("parser mode actions are balanced");
    }

    pub(crate) fn in_arith(&self) -> bool {
        matches!(self.modes.borrow().last(), Some(ParserMode::Arith))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ParserLocation(usize);
