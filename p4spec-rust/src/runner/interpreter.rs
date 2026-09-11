//! Stage-specific evaluation contract used by a composed runner.
//!
//! An interpreter defines the specification, configuration, and execution state
//! for one language stage. Evaluation receives the assembled runner context, so it
//! can call builtins and externs without storing callbacks or global state.

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
    type Config;
    type State: Default;
    type Error: From<InterfaceError> + From<ExternError>;

    /// Clears execution state without invalidating arena values
    fn clear(state: &mut Self::State);

    /// Evaluates an already parsed program through the selected entry
    fn eval_program(
        ctx: &mut RunnerContext<'_, Self, I, E>,
        name: &str,
        program: Value,
    ) -> Result<Vec<Value>, Self::Error>;

    fn eval_rel(
        ctx: &mut RunnerContext<'_, Self, I, E>,
        name: &str,
        values: &[Value],
    ) -> Result<Vec<Value>, Self::Error>;

    fn eval_func(
        ctx: &mut RunnerContext<'_, Self, I, E>,
        name: &str,
        targs: &[Typ],
        values: &[Value],
    ) -> Result<Value, Self::Error>;
}
