use std::rc::Rc;

use crate::{
    interp::common::InterpError,
    lang::il::ast::{Exp, ExpKind, Iter, Typ, Var},
    runtime::{
        dynamic::var::Variable,
        r#type::typ::make as make_type,
        value::{ValueKind, ValueRef, get, make},
    },
};

use super::{context::Context, expression::iterated_variable};

fn match_error(exp: &Exp) -> InterpError {
    InterpError::unmatch(exp.span.clone(), "match failed while assigning value")
}

// Assigning a value to an expression

pub fn assign(context: &mut Context, exp: &Exp, value: ValueRef) -> Result<(), InterpError> {
    let mark = context.mark();
    match assign_inner(context, exp, value) {
        Ok(()) => Ok(()),
        Err(error) => {
            context.reset(mark)?;
            Err(error)
        }
    }
}

fn assign_inner(context: &mut Context, exp: &Exp, value: ValueRef) -> Result<(), InterpError> {
    match (&exp.kind, &value.kind) {
        (ExpKind::VarE(id), _) => context.bind_value(Variable::new(id.clone(), Vec::new()), value),
        (ExpKind::TupleE(exps), ValueKind::TupleV(values)) => {
            assign_expressions(context, exps, values)
        }
        (ExpKind::CaseE(not_exp), ValueKind::CaseV(value_case)) => {
            let exps = not_exp.args();
            let values = value_case.args();
            assign_expression_refs(context, &exps, &values)
        }
        (ExpKind::StrE(exp_fields), ValueKind::StructV(value_fields)) => {
            if exp_fields.len() != value_fields.len() {
                return Err(match_error(exp));
            }
            for ((_, exp), (_, value)) in exp_fields.iter().zip(value_fields) {
                assign_inner(context, exp, Rc::clone(value))?;
            }
            Ok(())
        }
        (ExpKind::OptE(Some(exp)), ValueKind::OptV(Some(value))) => {
            assign_inner(context, exp, Rc::clone(value))
        }
        (ExpKind::OptE(None), ValueKind::OptV(None)) => Ok(()),
        (ExpKind::ListE(exps), ValueKind::ListV(values)) => {
            assign_expressions(context, exps, values)
        }
        (ExpKind::ConsE(head, tail), ValueKind::ListV(values)) => {
            let Some((value_head, values_tail)) = values.split_first() else {
                return Err(match_error(exp));
            };
            assign_inner(context, head, Rc::clone(value_head))?;
            let tail_type = Typ::new(value.ty.clone(), tail.span.clone());
            let value_tail = make::list(&tail_type, values_tail.to_vec(), tail.span.clone());
            assign_inner(context, tail, value_tail)
        }
        (ExpKind::IterE(exp_inner, (iter, vars)), _) => match iterated_variable(exp) {
            Some(variable) => context.bind_value(variable, value),
            None => assign_iterated_expression(context, exp_inner, *iter, vars, value),
        },
        _ => Err(match_error(exp)),
    }
}

fn assign_expressions(
    context: &mut Context,
    exps: &[Exp],
    values: &[ValueRef],
) -> Result<(), InterpError> {
    if exps.len() != values.len() {
        let span = exps
            .first()
            .map_or_else(crate::domain::source::Region::none, |exp| exp.span.clone());
        return Err(InterpError::unmatch(
            span,
            format!(
                "mismatch in number of expressions and values while assigning, expected {} value(s) but got {}",
                exps.len(),
                values.len()
            ),
        ));
    }
    for (exp, value) in exps.iter().zip(values) {
        assign_inner(context, exp, Rc::clone(value))?;
    }
    Ok(())
}

fn assign_expression_refs(
    context: &mut Context,
    exps: &[&Exp],
    values: &[&ValueRef],
) -> Result<(), InterpError> {
    if exps.len() != values.len() {
        return Err(InterpError::unmatch(
            crate::domain::source::Region::none(),
            "mismatch in number of expressions and values while assigning",
        ));
    }
    for (exp, value) in exps.iter().zip(values) {
        assign_inner(context, exp, Rc::clone(value))?;
    }
    Ok(())
}

fn collect_iterated_values(context: &Context, vars: &[Var]) -> Result<Vec<ValueRef>, InterpError> {
    vars.iter()
        .map(|(id, _typ, iters)| context.find_value_by(id, iters).map(Rc::clone))
        .collect()
}

fn assign_optional_iteration(
    context: &mut Context,
    exp: &Exp,
    vars: &[Var],
    value: ValueRef,
) -> Result<(), InterpError> {
    let value_opt = get::opt(&value)
        .map_err(|error| InterpError::new(value.span.region().clone(), error.to_string()))?;
    let values_inner = match value_opt {
        // Assign the value to the iterated expression.
        Some(value_inner) => Some(context.with_scope(|context| {
            assign_inner(context, exp, Rc::clone(value_inner))?;
            collect_iterated_values(context, vars)
        })?),
        None => None,
    };
    // Per iterated variable, make an option out of the value.
    for (index, (id, typ, iters)) in vars.iter().enumerate() {
        let mut outer_iters = iters.clone();
        outer_iters.push(Iter::Opt);
        let typ = make_type::iterate(typ.clone(), &outer_iters);
        let value_inner = values_inner
            .as_ref()
            .map(|values| Rc::clone(&values[index]));
        let value_outer = make::opt(&typ, value_inner, crate::domain::source::Region::none());
        context.bind_value(Variable::new(id.clone(), outer_iters), value_outer)?;
    }
    Ok(())
}

fn assign_list_iteration(
    context: &mut Context,
    exp: &Exp,
    vars: &[Var],
    value: ValueRef,
) -> Result<(), InterpError> {
    let values = get::list(&value)
        .map_err(|error| InterpError::new(value.span.region().clone(), error.to_string()))?;
    // Map over the value list elements, and assign each value to the
    // iterated expression in a cleared local context.
    let rows = values
        .iter()
        .map(|value_inner| {
            context.with_cleared_values(|context| {
                assign_inner(context, exp, Rc::clone(value_inner))?;
                collect_iterated_values(context, vars)
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Per iterated variable, collect its elementwise values, then make a
    // sequence out of them.
    for (index, (id, typ, iters)) in vars.iter().enumerate() {
        let mut outer_iters = iters.clone();
        outer_iters.push(Iter::List);
        let typ = make_type::iterate(typ.clone(), &outer_iters);
        let values_inner = rows.iter().map(|row| Rc::clone(&row[index])).collect();
        let value_outer = make::list(&typ, values_inner, crate::domain::source::Region::none());
        context.bind_value(Variable::new(id.clone(), outer_iters), value_outer)?;
    }
    Ok(())
}

fn assign_iterated_expression(
    context: &mut Context,
    exp: &Exp,
    iter: Iter,
    vars: &[Var],
    value: ValueRef,
) -> Result<(), InterpError> {
    match iter {
        Iter::Opt => assign_optional_iteration(context, exp, vars, value),
        Iter::List => assign_list_iteration(context, exp, vars, value),
    }
}
