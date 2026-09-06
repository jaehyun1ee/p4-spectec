//! Mutable state shared by the P4 lexer and parser.
//!
//! The context keeps a global scope followed by nested local scopes. A
//! top-level-only grammar production temporarily moves the local scopes aside,
//! then restores them without copying their namespace maps. For example, a
//! declaration parsed inside a control can inspect the global type namespace
//! and then resume resolving the control's parameters. Source positions are
//! interned so LALRPOP can use copyable indices while building spans.

use std::{cell::RefCell, collections::BTreeMap};

use crate::lang::common::source::{Position, Span};

use super::error::ContextError;

// == Names and scopes

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Location(usize);

pub type Namespace = BTreeMap<String, IdentKind>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeId {
    Empty,
    Local(String),
    Global(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdentKind {
    TypeName {
        has_params: bool,
        namespace: Namespace,
    },
    Ident {
        has_params: bool,
        type_id: TypeId,
    },
}

pub struct Context {
    /// Global namespace followed by the currently active local namespaces.
    scopes: RefCell<Vec<Namespace>>,
    /// Local namespaces set aside while parsing a top-level-only production.
    scopes_suspended: RefCell<Vec<Namespace>>,
    /// Most recently classified identifier, used to resolve a following member.
    id_prev: RefCell<Option<String>>,
    /// Namespace used to classify members of the most recent receiver.
    namespace_parent: RefCell<Option<Namespace>>,
    /// Source positions indexed by the copyable locations used by LALRPOP.
    positions: RefCell<Vec<Position>>,
}

// == Context operations

impl Context {
    // - Construction

    pub fn new() -> Self {
        Self {
            scopes: RefCell::new(vec![Namespace::new()]),
            scopes_suspended: RefCell::new(Vec::new()),
            id_prev: RefCell::new(None),
            namespace_parent: RefCell::new(None),
            positions: RefCell::new(Vec::new()),
        }
    }

    // - Declarations

    fn declare(&self, id: impl Into<String>, kind: IdentKind) -> Result<(), ContextError> {
        let mut scopes = self.scopes.borrow_mut();
        let scope = scopes.last_mut().ok_or(ContextError::MissingScope)?;
        scope.insert(id.into(), kind);
        Ok(())
    }

    pub fn declare_typ(&self, id: impl Into<String>, has_params: bool) -> Result<(), ContextError> {
        self.declare(
            id,
            IdentKind::TypeName {
                has_params,
                namespace: Namespace::new(),
            },
        )
    }

    pub fn declare_var(
        &self,
        id: impl Into<String>,
        has_params: bool,
        type_id: TypeId,
    ) -> Result<(), ContextError> {
        self.declare(
            id,
            IdentKind::Ident {
                has_params,
                type_id,
            },
        )
    }

    // - Identifier lookup

    fn ident_find(&self, id: &str) -> Option<IdentKind> {
        self.scopes
            .borrow()
            .iter()
            .rev()
            .find_map(|scope| scope.get(id).cloned())
    }

    pub fn ident_kind(&self, id: &str) -> IdentKind {
        let kind = match self.namespace_parent.borrow().as_ref() {
            Some(namespace) => namespace.get(id).cloned(),
            None => self.ident_find(id),
        }
        .unwrap_or(IdentKind::Ident {
            has_params: false,
            type_id: TypeId::Empty,
        });
        *self.id_prev.borrow_mut() = Some(id.to_owned());
        kind
    }

    // - Scope stack

    pub fn scope_push(&self) {
        self.scopes.borrow_mut().push(Namespace::new());
    }

    pub fn scope_pop(&self) -> Result<Namespace, ContextError> {
        let mut scopes = self.scopes.borrow_mut();
        if scopes.len() <= 1 {
            return Err(ContextError::RootScope);
        }
        scopes.pop().ok_or(ContextError::MissingScope)
    }

    pub fn scope_to_toplevel(&self) -> Result<(), ContextError> {
        let mut scopes = self.scopes.borrow_mut();
        if scopes.is_empty() {
            return Err(ContextError::MissingScope);
        }
        *self.scopes_suspended.borrow_mut() = scopes.split_off(1);
        Ok(())
    }

    pub fn scope_to_local(&self) {
        let mut scopes_suspended = self.scopes_suspended.borrow_mut();
        let mut scopes = self.scopes.borrow_mut();
        scopes.truncate(1);
        scopes.append(&mut scopes_suspended);
    }

    // - Namespaces

    pub fn namespace_set_typ(&self, id: &str, namespace: Namespace) {
        let mut scopes = self.scopes.borrow_mut();
        for scope in scopes.iter_mut().rev() {
            if let Some(IdentKind::TypeName {
                has_params,
                namespace: old_namespace,
            }) = scope.get_mut(id)
            {
                let _ = has_params;
                *old_namespace = namespace;
                return;
            }
        }
    }

    pub fn namespace_set_parent(&self) {
        let id_prev = self.id_prev.borrow().clone();
        let type_id = id_prev
            .as_deref()
            .and_then(|id| self.ident_find(id))
            .and_then(|kind| match kind {
                IdentKind::Ident { type_id, .. } => Some(type_id),
                IdentKind::TypeName { .. } => None,
            });

        let scopes = self.scopes.borrow();
        let find_namespace = |id: &str, scopes: &[Namespace]| {
            scopes.iter().rev().find_map(|scope| match scope.get(id) {
                Some(IdentKind::TypeName { namespace, .. }) => Some(namespace.clone()),
                _ => None,
            })
        };
        let namespace = type_id
            .and_then(|type_id| match type_id {
                TypeId::Empty => None,
                TypeId::Local(id) => find_namespace(&id, &scopes),
                TypeId::Global(id) => scopes
                    .first()
                    .and_then(|scope| find_namespace(&id, std::slice::from_ref(scope))),
            })
            .unwrap_or_default();
        *self.namespace_parent.borrow_mut() = Some(namespace);
    }

    pub fn namespace_clear_parent(&self) {
        self.namespace_parent.borrow_mut().take();
    }

    // - Source locations

    pub(crate) fn location_add(&self, position: Position) -> Location {
        let mut positions = self.positions.borrow_mut();
        let location = Location(positions.len());
        positions.push(position);
        location
    }

    pub(crate) fn location_get(&self, location: Location) -> Position {
        self.positions.borrow()[location.0].clone()
    }

    pub(crate) fn location_span(&self, location_l: Location, location_r: Location) -> Span {
        Span::new(self.location_get(location_l), self.location_get(location_r))
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}
