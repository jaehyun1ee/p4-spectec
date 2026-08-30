use std::{cell::RefCell, collections::BTreeMap};

use super::error::ContextError;

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
    scopes: RefCell<Vec<Namespace>>,
    backup: RefCell<Vec<Namespace>>,
    previous_id: RefCell<Option<String>>,
    parent_namespace: RefCell<Option<Namespace>>,
}

impl Context {
    pub fn new() -> Self {
        Self {
            scopes: RefCell::new(vec![Namespace::new()]),
            backup: RefCell::new(Vec::new()),
            previous_id: RefCell::new(None),
            parent_namespace: RefCell::new(None),
        }
    }

    pub fn reset(&self) {
        *self.scopes.borrow_mut() = vec![Namespace::new()];
        self.backup.borrow_mut().clear();
        self.previous_id.borrow_mut().take();
        self.parent_namespace.borrow_mut().take();
    }

    pub fn declare(&self, id: impl Into<String>, kind: IdentKind) -> Result<(), ContextError> {
        let mut scopes = self.scopes.borrow_mut();
        let scope = scopes.last_mut().ok_or(ContextError::MissingScope)?;
        scope.insert(id.into(), kind);
        Ok(())
    }

    pub fn declare_type(
        &self,
        id: impl Into<String>,
        has_params: bool,
    ) -> Result<(), ContextError> {
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

    pub fn find(&self, id: &str) -> Option<IdentKind> {
        find_in(id, &self.scopes.borrow())
    }

    pub fn get_kind(&self, id: &str) -> IdentKind {
        let kind = match self.parent_namespace.borrow().as_ref() {
            Some(namespace) => namespace.get(id).cloned(),
            None => self.find(id),
        }
        .unwrap_or(IdentKind::Ident {
            has_params: false,
            type_id: TypeId::Empty,
        });
        *self.previous_id.borrow_mut() = Some(id.to_owned());
        kind
    }

    pub fn is_type_name(&self, id: &str) -> bool {
        matches!(self.get_kind(id), IdentKind::TypeName { .. })
    }

    pub fn push_scope(&self) {
        self.scopes.borrow_mut().push(Namespace::new());
    }

    pub fn pop_scope(&self) -> Result<Namespace, ContextError> {
        let mut scopes = self.scopes.borrow_mut();
        if scopes.len() <= 1 {
            return Err(ContextError::RootScope);
        }
        scopes.pop().ok_or(ContextError::MissingScope)
    }

    pub fn go_toplevel(&self) -> Result<(), ContextError> {
        let scopes = self.scopes.borrow().clone();
        let global = scopes.first().cloned().ok_or(ContextError::MissingScope)?;
        *self.backup.borrow_mut() = scopes;
        *self.scopes.borrow_mut() = vec![global];
        Ok(())
    }

    pub fn go_local(&self) {
        let backup = self.backup.borrow().clone();
        if !backup.is_empty() {
            *self.scopes.borrow_mut() = backup;
        }
    }

    pub fn set_type_namespace(&self, id: &str, namespace: Namespace) {
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

    pub fn set_parent_namespace(&self) {
        let previous_id = self.previous_id.borrow().clone();
        let scopes = self.scopes.borrow();
        let namespace = previous_id
            .as_deref()
            .and_then(|id| find_in(id, &scopes))
            .and_then(|kind| match kind {
                IdentKind::Ident { type_id, .. } => Some(type_id),
                IdentKind::TypeName { .. } => None,
            })
            .and_then(|type_id| match type_id {
                TypeId::Empty => None,
                TypeId::Local(id) => find_type_namespace(&id, &scopes),
                TypeId::Global(id) => scopes
                    .first()
                    .and_then(|scope| find_type_namespace(&id, std::slice::from_ref(scope))),
            })
            .unwrap_or_default();
        *self.parent_namespace.borrow_mut() = Some(namespace);
    }

    pub fn clear_parent_namespace(&self) {
        self.parent_namespace.borrow_mut().take();
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

fn find_in(id: &str, scopes: &[Namespace]) -> Option<IdentKind> {
    scopes.iter().rev().find_map(|scope| scope.get(id).cloned())
}

fn find_type_namespace(id: &str, scopes: &[Namespace]) -> Option<Namespace> {
    match find_in(id, scopes) {
        Some(IdentKind::TypeName { namespace, .. }) => Some(namespace),
        _ => None,
    }
}
