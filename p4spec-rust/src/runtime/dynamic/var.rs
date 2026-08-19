use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    rc::Rc,
};

use hashbrown::Equivalent;

use crate::lang::il::ast::{Id, Iter};

#[derive(Clone, Debug)]
pub struct Variable {
    pub id: Rc<Id>,
    pub iters: Vec<Iter>,
}

impl Variable {
    pub fn new(id: Id, iters: Vec<Iter>) -> Self {
        Self {
            id: Rc::new(id),
            iters,
        }
    }
}

#[derive(Clone, Copy, Debug, Hash)]
pub struct VariableRef<'a> {
    id: &'a str,
    iters: &'a [Iter],
}

impl<'a> VariableRef<'a> {
    pub fn new(id: &'a str, iters: &'a [Iter]) -> Self {
        Self { id, iters }
    }
}

impl Equivalent<Variable> for VariableRef<'_> {
    fn equivalent(&self, variable: &Variable) -> bool {
        self.id == variable.id.node && self.iters == variable.iters
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

fn compare_iter(iter_a: Iter, iter_b: Iter) -> Ordering {
    match (iter_a, iter_b) {
        (Iter::Opt, Iter::Opt) | (Iter::List, Iter::List) => Ordering::Equal,
        (Iter::Opt, Iter::List) => Ordering::Less,
        (Iter::List, Iter::Opt) => Ordering::Greater,
    }
}

fn compare_iters(iters_a: &[Iter], iters_b: &[Iter]) -> Ordering {
    for (iter_a, iter_b) in iters_a.iter().zip(iters_b) {
        let ordering = compare_iter(*iter_a, *iter_b);
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    iters_a.len().cmp(&iters_b.len())
}

// Compare variables by id, then by iters

impl Ord for Variable {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id
            .node
            .cmp(&other.id.node)
            .then_with(|| compare_iters(&self.iters, &other.iters))
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
