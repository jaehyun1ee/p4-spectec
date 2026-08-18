use std::rc::Rc;

use crate::{
    interp::common::InterpError,
    lang::il::ast::{Exp, ExpKind, Typ},
    runtime::{
        dynamic::var::Variable,
        value::{ValueKind, ValueRef, make},
    },
};

use super::context::Context;

fn match_error(exp: &Exp) -> InterpError {
    InterpError::new(exp.span.clone(), "match failed while assigning value")
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
        (ExpKind::IterE(..), _) => Err(InterpError::new(
            exp.span.clone(),
            "iterated assignment is not implemented",
        )),
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
        return Err(InterpError::new(
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
        return Err(InterpError::new(
            crate::domain::source::Region::none(),
            "mismatch in number of expressions and values while assigning",
        ));
    }
    for (exp, value) in exps.iter().zip(values) {
        assign_inner(context, exp, Rc::clone(value))?;
    }
    Ok(())
}
