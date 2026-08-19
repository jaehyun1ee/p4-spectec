use crate::{
    interp::common::InterpError,
    lang::sl::ast::{Block, Instr, InstrKind},
    runtime::value::ValueRef,
};

use super::{
    context::Context,
    expression::{self, FunctionCalls},
};

#[derive(Clone, Debug)]
pub(crate) enum Flow {
    Continue,
    Result(Vec<ValueRef>),
    Return(ValueRef),
}

pub(crate) fn eval_instr(
    context: &mut Context,
    calls: &mut dyn FunctionCalls,
    instr: &Instr,
) -> Result<Flow, InterpError> {
    match &instr.kind {
        InstrKind::ResultI(_signature, exps) => {
            let values = exps
                .iter()
                .map(|exp| expression::eval_with_calls(context, calls, exp))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Flow::Result(values))
        }
        InstrKind::ReturnI(exp) => {
            expression::eval_with_calls(context, calls, exp).map(Flow::Return)
        }
        _ => Err(InterpError::new(
            instr.span.clone(),
            "instruction evaluation is not implemented",
        )),
    }
}

pub(crate) fn eval_block(
    context: &mut Context,
    calls: &mut dyn FunctionCalls,
    block: &Block,
) -> Result<Flow, InterpError> {
    for instr in block {
        match eval_instr(context, calls, instr)? {
            Flow::Continue => {}
            flow @ (Flow::Result(_) | Flow::Return(_)) => return Ok(flow),
        }
    }
    Ok(Flow::Continue)
}

pub(crate) fn return_value(
    flow: Flow,
    span: &crate::domain::source::Region,
) -> Result<ValueRef, InterpError> {
    match flow {
        Flow::Return(value) => Ok(value),
        Flow::Continue => Err(InterpError::new(
            span.clone(),
            "function did not return a value",
        )),
        Flow::Result(values) => {
            drop(values);
            Err(InterpError::new(
                span.clone(),
                "function cannot produce a relation result",
            ))
        }
    }
}
