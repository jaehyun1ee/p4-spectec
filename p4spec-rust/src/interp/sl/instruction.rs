use crate::{
    domain::source::{Region, Spanned},
    interp::common::InterpError,
    lang::{
        il::ast::{Exp, ExpKind, OpTyp, TypKind, UnOp},
        sl::ast::{Block, Guard, Instr, InstrKind},
    },
    runtime::{
        dynamic::var::Variable,
        value::{ValueRef, get},
    },
};

use super::{
    assignment,
    context::Context,
    expression::{self, Calls},
};

#[derive(Clone, Debug)]
pub(crate) enum Flow {
    Continue,
    Result(Vec<ValueRef>),
    Return(ValueRef),
}

pub(crate) fn eval_instr(
    context: &mut Context,
    calls: &mut dyn Calls,
    instr: &Instr,
) -> Result<Flow, InterpError> {
    match &instr.kind {
        InstrKind::IfI(condition, iter_exps, block, _dangle) => {
            if !iter_exps.is_empty() {
                return Err(InterpError::new(
                    instr.span.clone(),
                    "iterated if condition is not implemented",
                ));
            }
            let value = expression::eval_with_calls(context, calls, condition)?;
            let condition_holds = get::bool(&value)
                .map_err(|error| InterpError::new(condition.span.clone(), error.to_string()))?;
            if condition_holds {
                eval_block(context, calls, block)
            } else {
                Ok(Flow::Continue)
            }
        }
        InstrKind::CaseI(exp, cases, _dangle) => eval_case(context, calls, exp, cases),
        InstrKind::GroupI(_id, _signature, _exps, block) => eval_block(context, calls, block),
        InstrKind::LetI(left, right, iter_instrs, block) => {
            if !iter_instrs.is_empty() {
                return Err(InterpError::new(
                    instr.span.clone(),
                    "iterated let instruction is not implemented",
                ));
            }
            context.with_scope(|context| {
                let value = expression::eval_with_calls(context, calls, right)?;
                if let Err(error) = assignment::assign(context, left, value) {
                    return if error.is_unmatch() {
                        Ok(Flow::Continue)
                    } else {
                        Err(error)
                    };
                }
                eval_block(context, calls, block)
            })
        }
        InstrKind::RuleI(id, notation, inputs, iter_instrs, block) => {
            if !iter_instrs.is_empty() {
                return Err(InterpError::new(
                    instr.span.clone(),
                    "iterated rule instruction is not implemented",
                ));
            }
            context.with_scope(|context| {
                let exps = notation.args();
                let mut exps_input = Vec::new();
                let mut exps_output = Vec::new();
                for (index, exp) in exps.into_iter().enumerate() {
                    if inputs.contains(&(index as i64)) {
                        exps_input.push(exp);
                    } else {
                        exps_output.push(exp);
                    }
                }
                let values_input = exps_input
                    .into_iter()
                    .map(|exp| expression::eval_with_calls(context, calls, exp))
                    .collect::<Result<Vec<_>, _>>()?;
                let values_output = match calls.invoke_rel(context, id, &values_input) {
                    Ok(values) => values,
                    Err(error) if error.is_unmatch() => return Ok(Flow::Continue),
                    Err(error) => return Err(error),
                };
                if exps_output.len() != values_output.len() {
                    return Ok(Flow::Continue);
                }
                for (exp, value) in exps_output.into_iter().zip(values_output) {
                    if let Err(error) = assignment::assign(context, exp, value) {
                        if error.is_unmatch() {
                            return Ok(Flow::Continue);
                        }
                        return Err(error);
                    }
                }
                eval_block(context, calls, block)
            })
        }
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

fn guard_exp(exp: &Exp, guard: &Guard) -> Exp {
    let temporary = Exp::new(
        ExpKind::VarE(Spanned::new("~case".to_owned(), Region::none())),
        exp.ty.clone(),
        exp.span.clone(),
    );
    let kind = match guard {
        Guard::BoolG(true) => temporary.kind,
        Guard::BoolG(false) => ExpKind::UnE(UnOp::NotOp, OpTyp::BoolT, Box::new(temporary)),
        Guard::CmpG(operator, operator_type, right) => ExpKind::CmpE(
            *operator,
            *operator_type,
            Box::new(temporary),
            Box::new(right.clone()),
        ),
        Guard::SubG(typ) => ExpKind::SubE(Box::new(temporary), typ.clone()),
        Guard::MatchG(pattern) => ExpKind::MatchE(Box::new(temporary), pattern.clone()),
        Guard::MemG(collection) => ExpKind::MemE(Box::new(temporary), Box::new(collection.clone())),
    };
    Exp::new(kind, TypKind::BoolT, exp.span.clone())
}

fn eval_case(
    context: &mut Context,
    calls: &mut dyn Calls,
    exp: &Exp,
    cases: &[(Guard, Block)],
) -> Result<Flow, InterpError> {
    let value = expression::eval_with_calls(context, calls, exp)?;
    let temporary = Variable::new(Spanned::new("~case".to_owned(), Region::none()), Vec::new());
    context.with_value_bindings(vec![(temporary, value)], |context| {
        for (guard, block) in cases {
            let condition = guard_exp(exp, guard);
            let value = expression::eval_with_calls(context, calls, &condition)?;
            let matches = get::bool(&value)
                .map_err(|error| InterpError::new(condition.span.clone(), error.to_string()))?;
            if matches {
                return eval_block(context, calls, block);
            }
        }
        Ok(Flow::Continue)
    })
}

pub(crate) fn eval_block(
    context: &mut Context,
    calls: &mut dyn Calls,
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

pub(crate) fn result_values(
    flow: Flow,
    span: &crate::domain::source::Region,
) -> Result<Vec<ValueRef>, InterpError> {
    match flow {
        Flow::Result(values) => Ok(values),
        Flow::Continue => Err(InterpError::unmatch(
            span.clone(),
            "relation did not produce a result",
        )),
        Flow::Return(value) => {
            drop(value);
            Err(InterpError::new(
                span.clone(),
                "relation cannot return a value",
            ))
        }
    }
}
