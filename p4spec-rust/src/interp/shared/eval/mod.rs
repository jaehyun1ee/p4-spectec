//! Shared assignment, expression, argument, and path evaluation

pub(crate) mod arg;
pub mod assign;
pub(crate) mod expr;
pub(crate) mod ops;
pub(crate) mod path;

use super::{backtrack::Backtrack, context::Environment, error::Error};
use crate::{
    lang::{common::source::Span, data::value::Value, il::ast},
    runner::{Extern, Interface, Interpreter, RunnerContext},
};

// = Evaluation

pub(crate) trait Evaluator<Ctx, Iface, Exn>: Interpreter<Iface, Exn, Error = Error>
where
    Ctx: Environment,
    Iface: Interface,
    Exn: Extern,
{
    fn trace_exp(_exp: &ast::Exp, result: Backtrack<Value>) -> Backtrack<Value> {
        result
    }

    fn trace_arg(_arg: &ast::Arg, result: Backtrack<Value>) -> Backtrack<Value> {
        result
    }

    fn invoke_func(
        runner: &mut RunnerContext<'_, Self, Iface, Exn>,
        ctx: &Ctx,
        id: &ast::Id,
        targs: &[ast::Typ],
        values: &[Value],
    ) -> Backtrack<Value>;
    fn map_list(
        runner: &mut RunnerContext<'_, Self, Iface, Exn>,
        ctx: &Ctx,
        span: &Span,
        vars: &[ast::Var],
        eval: impl FnMut(&mut RunnerContext<'_, Self, Iface, Exn>, &Ctx) -> Backtrack<Value>,
    ) -> Backtrack<Vec<Value>>;
    fn map_opt(
        runner: &mut RunnerContext<'_, Self, Iface, Exn>,
        ctx: &Ctx,
        span: &Span,
        vars: &[ast::Var],
        eval: impl FnMut(&mut RunnerContext<'_, Self, Iface, Exn>, &Ctx) -> Backtrack<Value>,
    ) -> Backtrack<Option<Value>>;
}
