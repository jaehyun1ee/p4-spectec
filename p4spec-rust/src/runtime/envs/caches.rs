//! Standard hash-map caches used by dynamic evaluation
//!
//! Value computations key directly by immutable runtime values; function calls
//! key by a shared function name and argument slice. Inserting the same call
//! twice therefore replaces its previous result with normal `HashMap`
//! semantics.

use std::{collections::HashMap, rc::Rc};

use crate::lang::data::value::Value;

// == Value cache

pub type ValueCache<V> = HashMap<Rc<Value>, V>;

// == Call cache

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CallKey {
    pub id: Rc<str>,
    pub values: Rc<[Rc<Value>]>,
}

impl CallKey {
    pub fn new(id: impl Into<Rc<str>>, values: impl Into<Rc<[Rc<Value>]>>) -> Self {
        Self {
            id: id.into(),
            values: values.into(),
        }
    }
}

pub type CallCache<V> = HashMap<CallKey, V>;
