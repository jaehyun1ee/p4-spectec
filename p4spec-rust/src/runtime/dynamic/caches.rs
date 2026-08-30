use std::rc::Rc;

use crate::runtime::{cache::ClockCache, value::ValueRef};

pub type ValueCache<V> = ClockCache<ValueRef, V>;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CallKey {
    pub id: Rc<str>,
    pub values: Rc<[ValueRef]>,
}

impl CallKey {
    pub fn new(id: impl Into<Rc<str>>, values: impl Into<Rc<[ValueRef]>>) -> Self {
        Self {
            id: id.into(),
            values: values.into(),
        }
    }
}

pub type CallCache<V> = ClockCache<CallKey, V>;
