//! Successful call results and invocation-local effect tracking
//!
//! Memo keys belong to the runner's arena. Public entries clear memo tables;
//! active effect frames survive reentry and propagate taint back to callers

use std::collections::HashMap;

use crate::lang::data::value::{CanonId, Value, ValueArena, ValueKind};

// = Call identity

#[derive(Debug, PartialEq, Eq, Hash)]
pub(crate) struct CallKey {
    name: String,
    values: Vec<CanonId<ValueKind>>,
}

impl CallKey {
    // Type arguments and value annotations do not distinguish calls
    pub(crate) fn new(arena: &ValueArena, name: &str, values: &[Value]) -> Self {
        Self {
            name: name.to_owned(),
            values: values.iter().map(|value| arena.canon_id(value)).collect(),
        }
    }
}

// = Call cache

#[derive(Default)]
pub struct Cache {
    pub(crate) funcs: HashMap<CallKey, Value>,
    pub(crate) rels: HashMap<CallKey, Vec<Value>>,
    effects: Vec<bool>,
}

impl Cache {
    // - Lifecycle

    pub(crate) fn clear(&mut self) {
        self.funcs.clear();
        self.rels.clear();
    }

    // - Invocation effects

    pub(crate) fn begin(&mut self) {
        self.effects.push(false);
    }

    pub(crate) fn mark_effect(&mut self, side_effected: bool) {
        if let Some(effect) = self.effects.last_mut() {
            *effect |= side_effected;
        }
    }

    /// Finishes an invocation and reports whether it remained pure
    pub(crate) fn end(&mut self) -> bool {
        let side_effected = self.effects.pop().expect("active invocation frame");
        self.mark_effect(side_effected);
        !side_effected
    }
}
