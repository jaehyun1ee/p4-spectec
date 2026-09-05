//! Stage-specific evaluation contract used by a composed runner.
//!
//! An interpreter defines the specification and mutable execution state for
//! one language stage. Evaluation receives the assembled runner context, so it
//! can call builtins and externs without storing callbacks or global state.

use std::rc::Rc;

use crate::{
    lang::{data::value::Value, il::ast::Typ},
    runner::{ExternError, InterfaceError},
};

use super::{Extern, Interface, RunnerContext};

// == Interpreter contract

pub trait Interpreter<I, E>: Sized
where
    I: Interface,
    E: Extern,
{
    type Spec;
    type State;
    type Error: From<InterfaceError> + From<ExternError>;

    fn eval_rel(
        context: &mut RunnerContext<'_, Self, I, E>,
        name: &str,
        values: &[Rc<Value>],
    ) -> Result<Vec<Rc<Value>>, Self::Error>;

    fn eval_func(
        context: &mut RunnerContext<'_, Self, I, E>,
        name: &str,
        targs: &[Typ],
        values: &[Rc<Value>],
    ) -> Result<Rc<Value>, Self::Error>;

    fn clear(state: &mut Self::State);
}
