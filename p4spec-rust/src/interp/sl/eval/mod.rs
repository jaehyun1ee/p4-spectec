//! SL assignment, expression, instruction, and invocation evaluation

pub mod assign;
pub mod call;
pub mod expr;
pub mod instr;
pub mod iter;

use super::{SlInterp, context::Context};
use crate::interp::shared::error::ErrorKind;
use crate::{
    interp::shared::{backtrack::Backtrack, eval::Evaluator},
    lang::{common::source::Span, data::value::Value, sl::ast},
    runner::{Extern, Interface, RunnerContext},
};

impl<'global, Iface: Interface, Exn: Extern> Evaluator<Context<'global>, Iface, Exn> for SlInterp {
    fn trace_exp(exp: &ast::Exp, result: Backtrack<Value>) -> Backtrack<Value> {
        result.nest(exp.span.clone(), || {
            ErrorKind::Trace(crate::interp::shared::error::TraceErrorKind::Expression {
                exp: crate::lang::traits::print::Print::to_string(exp),
            })
        })
    }

    fn trace_arg(arg: &ast::Arg, result: Backtrack<Value>) -> Backtrack<Value> {
        result.nest(arg.span.clone(), || {
            ErrorKind::Trace(crate::interp::shared::error::TraceErrorKind::Expression {
                exp: crate::lang::traits::print::Print::to_string(arg),
            })
        })
    }

    fn invoke_func(
        runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
        ctx: &Context<'global>,
        id: &ast::Id,
        targs: &[ast::Typ],
        values: &[Value],
    ) -> Backtrack<Value> {
        call::invoke_func(runner, ctx, id, targs, values)
    }

    fn map_list(
        runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
        ctx: &Context<'global>,
        span: &Span,
        vars: &[ast::Var],
        eval: impl FnMut(
            &mut RunnerContext<'_, SlInterp, Iface, Exn>,
            &Context<'global>,
        ) -> Backtrack<Value>,
    ) -> Backtrack<Vec<Value>> {
        iter::map_list(runner, ctx, span, vars, eval)
    }

    fn map_opt(
        runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
        ctx: &Context<'global>,
        span: &Span,
        vars: &[ast::Var],
        eval: impl FnMut(
            &mut RunnerContext<'_, SlInterp, Iface, Exn>,
            &Context<'global>,
        ) -> Backtrack<Value>,
    ) -> Backtrack<Option<Value>> {
        iter::map_opt(runner, ctx, span, vars, eval)
    }
}
