use std::rc::Rc;

use num_bigint::BigInt;
use num_traits::Zero;

use crate::{
    domain::source::Region,
    interp::common::InterpError,
    lang::{
        il::ast::{BinOp, CmpOp, Exp, ExpKind, UnOp},
        xl::num,
    },
    runtime::{
        dynamic::var::Variable,
        value::{Value, ValueRef, get, make},
    },
};

use super::context::Context;

fn value_error(exp: &Exp, message: impl Into<String>) -> InterpError {
    InterpError::new(exp.span.clone(), message)
}

fn bool_of_value(exp: &Exp, value: &Value) -> Result<bool, InterpError> {
    get::bool(value).map_err(|error| value_error(exp, error.to_string()))
}

fn num_of_value<'a>(exp: &Exp, value: &'a Value) -> Result<&'a num::T, InterpError> {
    get::num(value).map_err(|error| value_error(exp, error.to_string()))
}

// Expression evaluation

pub fn eval(context: &Context, exp: &Exp) -> Result<ValueRef, InterpError> {
    match &exp.kind {
        ExpKind::BoolE(value) => Ok(make::bool(*value, Region::none())),
        ExpKind::NumE(value) => Ok(make::num(value.clone(), Region::none())),
        ExpKind::TextE(value) => Ok(make::text(value.clone(), Region::none())),
        ExpKind::VarE(id) => context
            .find_value(&Variable::new(id.clone(), Vec::new()))
            .map(Rc::clone),
        ExpKind::UnE(operator, _operator_type, operand) => {
            eval_unary(context, exp, *operator, operand)
        }
        ExpKind::BinE(operator, _operator_type, left, right) => {
            eval_binary(context, exp, *operator, left, right)
        }
        ExpKind::CmpE(operator, _operator_type, left, right) => {
            eval_comparison(context, exp, *operator, left, right)
        }
        _ => Err(value_error(exp, "expression evaluation is not implemented")),
    }
}

// Unary expression evaluation

fn eval_unary(
    context: &Context,
    outer: &Exp,
    operator: UnOp,
    operand: &Exp,
) -> Result<ValueRef, InterpError> {
    let value = eval(context, operand)?;
    match operator {
        UnOp::NotOp => Ok(make::bool(!bool_of_value(outer, &value)?, Region::none())),
        UnOp::PlusOp => Ok(make::num(
            num_of_value(outer, &value)?.clone(),
            Region::none(),
        )),
        UnOp::MinusOp => {
            let value = match num_of_value(outer, &value)? {
                num::T::Nat(value) | num::T::Int(value) => num::T::Int(-value),
            };
            Ok(make::num(value, Region::none()))
        }
    }
}

// Binary expression evaluation

fn eval_binary(
    context: &Context,
    outer: &Exp,
    operator: BinOp,
    left: &Exp,
    right: &Exp,
) -> Result<ValueRef, InterpError> {
    // OCaml evaluates both operands before dispatching the operator.
    let value_left = eval(context, left)?;
    let value_right = eval(context, right)?;
    match operator {
        BinOp::AndOp | BinOp::OrOp | BinOp::ImplOp | BinOp::EquivOp => {
            let left = bool_of_value(outer, &value_left)?;
            let right = bool_of_value(outer, &value_right)?;
            let result = match operator {
                BinOp::AndOp => left && right,
                BinOp::OrOp => left || right,
                BinOp::ImplOp => !left || right,
                BinOp::EquivOp => left == right,
                _ => unreachable!(),
            };
            Ok(make::bool(result, Region::none()))
        }
        BinOp::AddOp | BinOp::SubOp | BinOp::MulOp | BinOp::DivOp | BinOp::ModOp | BinOp::PowOp => {
            let left = num_of_value(outer, &value_left)?;
            let right = num_of_value(outer, &value_right)?;
            eval_binary_num(outer, operator, left, right)
        }
    }
}

fn eval_binary_num(
    outer: &Exp,
    operator: BinOp,
    left: &num::T,
    right: &num::T,
) -> Result<ValueRef, InterpError> {
    if operator == BinOp::PowOp {
        return Err(value_error(outer, "numeric power is not implemented"));
    }
    let result = match (left, right) {
        (num::T::Nat(left), num::T::Nat(right)) => {
            eval_binary_bigint(outer, operator, left, right, true)?
        }
        (num::T::Int(left), num::T::Int(right)) => {
            eval_binary_bigint(outer, operator, left, right, false)?
        }
        _ => {
            return Err(value_error(
                outer,
                "numeric operands have different runtime types",
            ));
        }
    };
    Ok(make::num(result, Region::none()))
}

fn eval_binary_bigint(
    outer: &Exp,
    operator: BinOp,
    left: &BigInt,
    right: &BigInt,
    natural: bool,
) -> Result<num::T, InterpError> {
    let value = match operator {
        BinOp::AddOp => left + right,
        BinOp::SubOp => left - right,
        BinOp::MulOp => left * right,
        BinOp::DivOp | BinOp::ModOp if right.is_zero() => {
            return Err(value_error(outer, "division by zero"));
        }
        BinOp::DivOp => left / right,
        BinOp::ModOp => left % right,
        BinOp::PowOp | BinOp::AndOp | BinOp::OrOp | BinOp::ImplOp | BinOp::EquivOp => {
            unreachable!()
        }
    };
    if natural && operator != BinOp::SubOp {
        Ok(num::T::Nat(value))
    } else {
        Ok(num::T::Int(value))
    }
}

// Comparison expression evaluation

fn eval_comparison(
    context: &Context,
    outer: &Exp,
    operator: CmpOp,
    left: &Exp,
    right: &Exp,
) -> Result<ValueRef, InterpError> {
    let value_left = eval(context, left)?;
    let value_right = eval(context, right)?;
    let result = match operator {
        CmpOp::EqOp => value_left == value_right,
        CmpOp::NeOp => value_left != value_right,
        CmpOp::LtOp | CmpOp::GtOp | CmpOp::LeOp | CmpOp::GeOp => {
            let left = num_of_value(outer, &value_left)?;
            let right = num_of_value(outer, &value_right)?;
            match (left, right) {
                (num::T::Nat(left), num::T::Nat(right))
                | (num::T::Int(left), num::T::Int(right)) => match operator {
                    CmpOp::LtOp => left < right,
                    CmpOp::GtOp => left > right,
                    CmpOp::LeOp => left <= right,
                    CmpOp::GeOp => left >= right,
                    _ => unreachable!(),
                },
                _ => {
                    return Err(value_error(
                        outer,
                        "numeric operands have different runtime types",
                    ));
                }
            }
        }
    };
    Ok(make::bool(result, Region::none()))
}
