use std::rc::Rc;

use crate::{
    domain::source::{Region, Spanned},
    interp::common::InterpError,
    lang::{
        il::ast::{Exp, ExpKind, Id, OpTyp, TypKind, UnOp},
        sl::ast::{Block, Guard, HoldCase, Instr, InstrKind, IterExp, IterInstr, NotExp, TableRow},
    },
    runtime::{
        dynamic::var::Variable,
        r#type::typ::make as make_type,
        value::{ValueRef, get, make},
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
            let condition_holds = eval_if_condition(context, calls, condition, iter_exps)?;
            if condition_holds {
                eval_block(context, calls, block)
            } else {
                Ok(Flow::Continue)
            }
        }
        InstrKind::CaseI(exp, cases, _dangle) => eval_case(context, calls, exp, cases),
        InstrKind::HoldI(id, notation, iter_exps, hold_case) => {
            let holds = eval_hold_condition(context, calls, id, notation, iter_exps)?;
            match hold_case {
                HoldCase::BothH(block_hold, block_not_hold) => {
                    if holds {
                        eval_block(context, calls, block_hold)
                    } else {
                        eval_block(context, calls, block_not_hold)
                    }
                }
                HoldCase::HoldH(block, _dangle) if holds => eval_block(context, calls, block),
                HoldCase::NotHoldH(block, _dangle) if !holds => eval_block(context, calls, block),
                HoldCase::HoldH(..) | HoldCase::NotHoldH(..) => Ok(Flow::Continue),
            }
        }
        InstrKind::GroupI(_id, _signature, _exps, block) => eval_block(context, calls, block),
        InstrKind::LetI(left, right, iter_instrs, block) => context.with_scope(|context| {
            let result = (|| {
                eval_let_iter(context, calls, left, right, iter_instrs)?;
                eval_block(context, calls, block)
            })();
            match result {
                Err(error) if error.is_unmatch() => Ok(Flow::Continue),
                result => result,
            }
        }),
        InstrKind::RuleI(id, notation, inputs, iter_instrs, block) => {
            context.with_scope(|context| {
                let result = (|| {
                    eval_rule_iter(context, calls, id, notation, inputs, iter_instrs)?;
                    eval_block(context, calls, block)
                })();
                match result {
                    Err(error) if error.is_unmatch() => Ok(Flow::Continue),
                    result => result,
                }
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
        InstrKind::DebugI(exp, nested) => {
            let result = (|| {
                let value = expression::eval_with_calls(context, calls, exp)?;
                println!("{}: {:?}", exp.span, exp.kind);
                if value.span == Region::none() {
                    println!("{:?}", value.kind);
                } else {
                    println!("{}: {:?}", value.span, value.kind);
                }
                eval_instr(context, calls, nested)
            })();
            match result {
                Err(error) if error.is_unmatch() => Ok(Flow::Continue),
                result => result,
            }
        }
    }
}

// Let instruction evaluation

fn eval_let(
    context: &mut Context,
    calls: &mut dyn Calls,
    left: &Exp,
    right: &Exp,
) -> Result<(), InterpError> {
    let value = expression::eval_with_calls(context, calls, right)?;
    assignment::assign(context, left, value)
}

fn eval_let_opt(
    context: &mut Context,
    calls: &mut dyn Calls,
    left: &Exp,
    right: &Exp,
    vars_bound: &[crate::lang::il::ast::Var],
    vars_bind: &[crate::lang::il::ast::Var],
    iter_instrs: &[IterInstr],
) -> Result<(), InterpError> {
    let bindings = context.optional_bindings(vars_bound)?;
    let values_binding = match bindings {
        // If the bound variable supposed to guide the iteration is already empty,
        // then the binding variables are also empty.
        None => vec![None; vars_bind.len()],
        // Otherwise, evaluate the premise for the subcontext.
        Some(bindings) => context.with_value_bindings(bindings, |context| {
            eval_let_iter_inner(context, calls, left, right, iter_instrs)?;
            vars_bind
                .iter()
                .map(|(id, _typ, iters)| {
                    context
                        .find_value(&Variable::new(id.clone(), iters.clone()))
                        .map(|value| Some(Rc::clone(value)))
                })
                .collect::<Result<Vec<_>, _>>()
        })?,
    };

    // Finally, bind the resulting values.
    for ((id, typ, iters), value) in vars_bind.iter().zip(values_binding) {
        let mut outer_iters = iters.clone();
        outer_iters.push(crate::lang::il::ast::Iter::Opt);
        let outer_type = make_type::iterate(typ.clone(), &outer_iters);
        let value = make::opt(&outer_type, value, Region::none());
        context.bind_value(Variable::new(id.clone(), outer_iters), value)?;
    }
    Ok(())
}

fn eval_let_list(
    context: &mut Context,
    calls: &mut dyn Calls,
    left: &Exp,
    right: &Exp,
    vars_bound: &[crate::lang::il::ast::Var],
    vars_bind: &[crate::lang::il::ast::Var],
    iter_instrs: &[IterInstr],
) -> Result<(), InterpError> {
    // Create a subcontext for each batch of bound values.
    let bindings_batches = context.list_binding_batches(vars_bound)?;
    let mut values_binding_batches = Vec::with_capacity(bindings_batches.len());
    for bindings in bindings_batches {
        // Evaluate the premise for each batch of bound values, and collect the
        // resulting binding batches.
        let values = context.with_value_bindings(bindings, |context| {
            eval_let_iter_inner(context, calls, left, right, iter_instrs)?;
            vars_bind
                .iter()
                .map(|(id, _typ, iters)| {
                    context
                        .find_value(&Variable::new(id.clone(), iters.clone()))
                        .map(Rc::clone)
                })
                .collect::<Result<Vec<_>, _>>()
        })?;
        values_binding_batches.push(values);
    }

    // Finally, bind the resulting binding batches.
    for (index, (id, typ, iters)) in vars_bind.iter().enumerate() {
        let values = values_binding_batches
            .iter()
            .map(|values| Rc::clone(&values[index]))
            .collect();
        let mut outer_iters = iters.clone();
        outer_iters.push(crate::lang::il::ast::Iter::List);
        let outer_type = make_type::iterate(typ.clone(), &outer_iters);
        let value = make::list(&outer_type, values, Region::none());
        context.bind_value(Variable::new(id.clone(), outer_iters), value)?;
    }
    Ok(())
}

fn eval_let_iter_inner(
    context: &mut Context,
    calls: &mut dyn Calls,
    left: &Exp,
    right: &Exp,
    iter_instrs: &[IterInstr],
) -> Result<(), InterpError> {
    let Some(((iter, vars_bound, vars_bind), rest)) = iter_instrs.split_first() else {
        return eval_let(context, calls, left, right);
    };
    match iter {
        crate::lang::il::ast::Iter::Opt => {
            eval_let_opt(context, calls, left, right, vars_bound, vars_bind, rest)
        }
        crate::lang::il::ast::Iter::List => {
            eval_let_list(context, calls, left, right, vars_bound, vars_bind, rest)
        }
    }
}

fn eval_let_iter(
    context: &mut Context,
    calls: &mut dyn Calls,
    left: &Exp,
    right: &Exp,
    iter_instrs: &[IterInstr],
) -> Result<(), InterpError> {
    let iter_instrs = iter_instrs.iter().cloned().rev().collect::<Vec<_>>();
    eval_let_iter_inner(context, calls, left, right, &iter_instrs)
}

// Rule instruction evaluation

fn eval_rule(
    context: &mut Context,
    calls: &mut dyn Calls,
    id: &Id,
    notation: &NotExp,
    inputs: &[i64],
) -> Result<(), InterpError> {
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
    let values_output = calls.invoke_rel(context, id, &values_input)?;
    if exps_output.len() != values_output.len() {
        return Err(InterpError::unmatch(
            id.span.clone(),
            "relation output arity does not match rule outputs",
        ));
    }
    for (exp, value) in exps_output.into_iter().zip(values_output) {
        assignment::assign(context, exp, value)?;
    }
    Ok(())
}

// Keep the OCaml premise components explicit so the port remains auditable.
#[allow(clippy::too_many_arguments)]
fn eval_rule_opt(
    context: &mut Context,
    calls: &mut dyn Calls,
    id: &Id,
    notation: &NotExp,
    inputs: &[i64],
    vars_bound: &[crate::lang::il::ast::Var],
    vars_bind: &[crate::lang::il::ast::Var],
    iter_instrs: &[IterInstr],
) -> Result<(), InterpError> {
    // Create a subcontext for the bound values.
    let bindings = context.optional_bindings(vars_bound)?;
    let values_binding = match bindings {
        // If the bound variable supposed to guide the iteration is already empty,
        // then the binding variables are also empty.
        None => vec![None; vars_bind.len()],
        // Otherwise, evaluate the rule for the subcontext.
        Some(bindings) => context.with_value_bindings(bindings, |context| {
            eval_rule_iter_inner(context, calls, id, notation, inputs, iter_instrs)?;
            vars_bind
                .iter()
                .map(|(id, _typ, iters)| {
                    context
                        .find_value(&Variable::new(id.clone(), iters.clone()))
                        .map(|value| Some(Rc::clone(value)))
                })
                .collect::<Result<Vec<_>, _>>()
        })?,
    };

    for ((id, typ, iters), value) in vars_bind.iter().zip(values_binding) {
        let mut outer_iters = iters.clone();
        outer_iters.push(crate::lang::il::ast::Iter::Opt);
        let outer_type = make_type::iterate(typ.clone(), &outer_iters);
        let value = make::opt(&outer_type, value, Region::none());
        context.bind_value(Variable::new(id.clone(), outer_iters), value)?;
    }
    Ok(())
}

// Keep the OCaml premise components explicit so the port remains auditable.
#[allow(clippy::too_many_arguments)]
fn eval_rule_list(
    context: &mut Context,
    calls: &mut dyn Calls,
    id: &Id,
    notation: &NotExp,
    inputs: &[i64],
    vars_bound: &[crate::lang::il::ast::Var],
    vars_bind: &[crate::lang::il::ast::Var],
    iter_instrs: &[IterInstr],
) -> Result<(), InterpError> {
    // Create a subcontext for each batch of bound values.
    let bindings_batches = context.list_binding_batches(vars_bound)?;
    let mut values_binding_batches = Vec::with_capacity(bindings_batches.len());
    for bindings in bindings_batches {
        // Evaluate the premise for each batch of bound values, and collect the
        // resulting binding batches.
        let values = context.with_value_bindings(bindings, |context| {
            eval_rule_iter_inner(context, calls, id, notation, inputs, iter_instrs)?;
            vars_bind
                .iter()
                .map(|(id, _typ, iters)| {
                    context
                        .find_value(&Variable::new(id.clone(), iters.clone()))
                        .map(Rc::clone)
                })
                .collect::<Result<Vec<_>, _>>()
        })?;
        values_binding_batches.push(values);
    }

    // Finally, bind the resulting binding batches.
    for (index, (id, typ, iters)) in vars_bind.iter().enumerate() {
        let values = values_binding_batches
            .iter()
            .map(|values| Rc::clone(&values[index]))
            .collect();
        let mut outer_iters = iters.clone();
        outer_iters.push(crate::lang::il::ast::Iter::List);
        let outer_type = make_type::iterate(typ.clone(), &outer_iters);
        let value = make::list(&outer_type, values, Region::none());
        context.bind_value(Variable::new(id.clone(), outer_iters), value)?;
    }
    Ok(())
}

fn eval_rule_iter_inner(
    context: &mut Context,
    calls: &mut dyn Calls,
    id: &Id,
    notation: &NotExp,
    inputs: &[i64],
    iter_instrs: &[IterInstr],
) -> Result<(), InterpError> {
    let Some(((iter, vars_bound, vars_bind), rest)) = iter_instrs.split_first() else {
        return eval_rule(context, calls, id, notation, inputs);
    };
    match iter {
        crate::lang::il::ast::Iter::Opt => eval_rule_opt(
            context, calls, id, notation, inputs, vars_bound, vars_bind, rest,
        ),
        crate::lang::il::ast::Iter::List => eval_rule_list(
            context, calls, id, notation, inputs, vars_bound, vars_bind, rest,
        ),
    }
}

fn eval_rule_iter(
    context: &mut Context,
    calls: &mut dyn Calls,
    id: &Id,
    notation: &NotExp,
    inputs: &[i64],
    iter_instrs: &[IterInstr],
) -> Result<(), InterpError> {
    let iter_instrs = iter_instrs.iter().cloned().rev().collect::<Vec<_>>();
    eval_rule_iter_inner(context, calls, id, notation, inputs, &iter_instrs)
}

fn eval_if_condition(
    context: &mut Context,
    calls: &mut dyn Calls,
    condition: &Exp,
    iter_exps: &[IterExp],
) -> Result<bool, InterpError> {
    let Some(((iter, vars), rest)) = iter_exps.split_last() else {
        let value = expression::eval_with_calls(context, calls, condition)?;
        return get::bool(&value)
            .map_err(|error| InterpError::new(condition.span.clone(), error.to_string()));
    };
    match iter {
        crate::lang::il::ast::Iter::Opt => match context.optional_bindings(vars)? {
            Some(bindings) => context.with_value_bindings(bindings, |context| {
                eval_if_condition(context, calls, condition, rest)
            }),
            None => Ok(false),
        },
        crate::lang::il::ast::Iter::List => {
            let batches = context.list_binding_batches(vars)?;
            for bindings in batches {
                let holds = context.with_value_bindings(bindings, |context| {
                    eval_if_condition(context, calls, condition, rest)
                })?;
                if !holds {
                    return Ok(false);
                }
            }
            Ok(true)
        }
    }
}

// Hold instruction evaluation

fn eval_hold_condition_inner(
    context: &mut Context,
    calls: &mut dyn Calls,
    id: &Id,
    notation: &NotExp,
    iter_exps: &[IterExp],
) -> Result<bool, InterpError> {
    let Some(((iter, vars), rest)) = iter_exps.split_first() else {
        let values_input = notation
            .args()
            .into_iter()
            .map(|exp| expression::eval_with_calls(context, calls, exp))
            .collect::<Result<Vec<_>, _>>()?;
        return match calls.invoke_rel(context, id, &values_input) {
            Ok(_values) => Ok(true),
            Err(error) if error.is_unmatch() => Ok(false),
            Err(error) => Err(error),
        };
    };
    match iter {
        crate::lang::il::ast::Iter::Opt => match context.optional_bindings(vars)? {
            Some(bindings) => context.with_value_bindings(bindings, |context| {
                eval_hold_condition_inner(context, calls, id, notation, rest)
            }),
            None => Ok(false),
        },
        crate::lang::il::ast::Iter::List => {
            let batches = context.list_binding_batches(vars)?;
            for bindings in batches {
                let holds = context.with_value_bindings(bindings, |context| {
                    eval_hold_condition_inner(context, calls, id, notation, rest)
                })?;
                if !holds {
                    return Ok(false);
                }
            }
            Ok(true)
        }
    }
}

fn eval_hold_condition(
    context: &mut Context,
    calls: &mut dyn Calls,
    id: &Id,
    notation: &NotExp,
    iter_exps: &[IterExp],
) -> Result<bool, InterpError> {
    let iter_exps = iter_exps.iter().cloned().rev().collect::<Vec<_>>();
    eval_hold_condition_inner(context, calls, id, notation, &iter_exps)
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
    if context.deterministic() {
        eval_block_deterministic(context, calls, block)
    } else {
        eval_block_sequential(context, calls, block)
    }
}

pub(crate) fn eval_block_sequential(
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

pub(crate) fn eval_table_rows(
    context: &mut Context,
    calls: &mut dyn Calls,
    rows: &[TableRow],
) -> Result<Flow, InterpError> {
    for (_inputs, _output, block) in rows {
        match eval_block_sequential(context, calls, block)? {
            Flow::Continue => {}
            flow @ (Flow::Result(_) | Flow::Return(_)) => return Ok(flow),
        }
    }
    Ok(Flow::Continue)
}

fn eval_block_deterministic(
    context: &mut Context,
    calls: &mut dyn Calls,
    block: &Block,
) -> Result<Flow, InterpError> {
    let mut selected = Flow::Continue;
    for instr in block {
        let flow = match context.with_scope(|context| eval_instr(context, calls, instr)) {
            Ok(flow) => flow,
            Err(error) if error.is_unmatch() => Flow::Continue,
            Err(error) => return Err(error),
        };
        selected = match (selected, flow) {
            (Flow::Continue, flow) | (flow, Flow::Continue) => flow,
            (Flow::Result(_), Flow::Return(_)) | (Flow::Return(_), Flow::Result(_)) => {
                return Err(InterpError::new(
                    instr.span.clone(),
                    "cannot have both result and return",
                ));
            }
            (Flow::Result(_), Flow::Result(_)) | (Flow::Return(_), Flow::Return(_)) => {
                return Err(InterpError::new(
                    instr.span.clone(),
                    "nondeterministic instruction evaluation",
                ));
            }
        };
    }
    Ok(selected)
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
