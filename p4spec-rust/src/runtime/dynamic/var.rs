use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
};

use crate::lang::il::ast::{Id, Iter};

#[derive(Clone, Debug)]
pub struct Variable {
    pub id: Id,
    pub iters: Vec<Iter>,
}

impl Variable {
    pub fn new(id: Id, iters: Vec<Iter>) -> Self {
        Self { id, iters }
    }
}

impl fmt::Display for Variable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.id.node)?;
        for iter in &self.iters {
            formatter.write_str(match iter {
                Iter::Opt => "?",
                Iter::List => "*",
            })?;
        }
        Ok(())
    }
}

impl Ord for Variable {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id
            .node
            .cmp(&other.id.node)
            .then_with(|| self.iters.cmp(&other.iters))
    }
}

impl PartialOrd for Variable {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Variable {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Variable {}

impl Hash for Variable {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.node.hash(state);
        self.iters.hash(state);
    }
}
