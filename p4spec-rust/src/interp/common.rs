//! Observation boundaries shared by specification interpreters

use crate::lang::{
    common::{Id, source::Span},
    data::value::Value,
};
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Program(Rc<Value>),
    FuncEnter {
        id: Id,
        values: Vec<Rc<Value>>,
    },
    FuncExit {
        id: Id,
    },
    RelEnter {
        id: Id,
        values: Vec<Rc<Value>>,
    },
    RelExit {
        id: Id,
    },
    Debug {
        span: Span,
        expression: String,
        value: Rc<Value>,
    },
}

pub trait Observer {
    fn event(&mut self, event: &Event);
}
