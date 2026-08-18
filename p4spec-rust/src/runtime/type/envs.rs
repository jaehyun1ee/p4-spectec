use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap},
    hash::{Hash, Hasher},
};

use crate::lang::il::ast::Id;

use super::typdef::TypeDef;

// Type definition environment

#[derive(Clone, Debug)]
struct TypeId(Id);

impl PartialEq for TypeId {
    fn eq(&self, other: &Self) -> bool {
        self.0.node == other.0.node
    }
}

impl Eq for TypeId {}

impl PartialOrd for TypeId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TypeId {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.node.cmp(&other.0.node)
    }
}

impl Hash for TypeId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.node.hash(state);
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TypeDefEnv(BTreeMap<TypeId, TypeDef>);

impl TypeDefEnv {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, id: Id, type_def: TypeDef) -> Option<TypeDef> {
        let previous = self.0.remove(&TypeId(id.clone()));
        self.0.insert(TypeId(id), type_def);
        previous
    }

    pub fn get(&self, id: &Id) -> Option<&TypeDef> {
        self.0.get(&TypeId(id.clone()))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Id, &TypeDef)> {
        self.0.iter().map(|(id, type_def)| (&id.0, type_def))
    }
}

#[derive(Clone, Debug, Default)]
pub struct TypeDefTable {
    bindings: HashMap<TypeId, Vec<TypeDef>>,
    len: usize,
}

impl TypeDefTable {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            bindings: HashMap::with_capacity(capacity),
            len: 0,
        }
    }

    pub fn add(&mut self, id: Id, type_def: TypeDef) {
        self.bindings.entry(TypeId(id)).or_default().push(type_def);
        self.len += 1;
    }

    pub fn get(&self, id: &Id) -> Option<&TypeDef> {
        self.bindings
            .get(&TypeId(id.clone()))
            .and_then(|bindings| bindings.last())
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}
