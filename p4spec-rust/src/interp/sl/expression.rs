use std::rc::Rc;

use num_bigint::BigInt;
use num_traits::{ToPrimitive, Zero};

use crate::{
    domain::source::Region,
    interp::common::InterpError,
    lang::{
        il::ast::{
            Arg, ArgKind, BinOp, CmpOp, DefTypKind, Exp, ExpKind, Iter, ListPattern, OptPattern,
            Path, PathKind, Pattern, Typ, TypKind, UnOp,
        },
        xl::num,
    },
    runtime::{
        dynamic::var::Variable,
        r#type::{
            subst::{self, TypeSubstitution},
            typdef::TypeDef,
        },
        value::{Value, ValueKind, ValueRef, get, make, r#match as value_match},
    },
};

use super::context::Context;

#[cfg(test)]
mod tests;

fn value_error(exp: &Exp, message: impl Into<String>) -> InterpError {
    InterpError::new(exp.span.clone(), message)
}

fn path_error(path: &Path, message: impl Into<String>) -> InterpError {
    InterpError::new(path.span.clone(), message)
}

fn bool_of_value(exp: &Exp, value: &Value) -> Result<bool, InterpError> {
    get::bool(value).map_err(|error| value_error(exp, error.to_string()))
}

fn num_of_value<'a>(exp: &Exp, value: &'a Value) -> Result<&'a num::T, InterpError> {
    get::num(value).map_err(|error| value_error(exp, error.to_string()))
}

// Helper for checking if an expression is a simple iteration of a variable

pub(crate) fn iterated_variable(exp: &Exp) -> Option<Variable> {
    match &exp.kind {
        ExpKind::VarE(id) => Some(Variable::new(id.clone(), Vec::new())),
        ExpKind::IterE(inner, (iter, vars)) => {
            let mut variable = iterated_variable(inner)?;
            let [(id, _typ, iters)] = vars.as_slice() else {
                return None;
            };
            if variable.id.node != id.node || variable.iters != *iters {
                return None;
            }
            variable.iters.push(*iter);
            Some(variable)
        }
        _ => None,
    }
}

pub(crate) trait Calls {
    fn value_is_subtype(
        &mut self,
        context: &Context,
        typ: &Typ,
        value: &ValueRef,
    ) -> Result<bool, InterpError> {
        value_is_subtype_uncached(context, typ, value)
    }

    fn invoke_func(
        &mut self,
        context: &mut Context,
        id: &crate::lang::il::ast::Id,
        type_args: &[Typ],
        values: &[ValueRef],
    ) -> Result<ValueRef, InterpError>;

    fn invoke_rel(
        &mut self,
        _context: &mut Context,
        id: &crate::lang::il::ast::Id,
        _values: &[ValueRef],
    ) -> Result<Vec<ValueRef>, InterpError> {
        Err(InterpError::new(
            id.span.clone(),
            "relation calls require an SL interpreter",
        ))
    }
}

struct RejectCalls;

impl Calls for RejectCalls {
    fn invoke_func(
        &mut self,
        _context: &mut Context,
        id: &crate::lang::il::ast::Id,
        _type_args: &[Typ],
        _values: &[ValueRef],
    ) -> Result<ValueRef, InterpError> {
        Err(InterpError::new(
            id.span.clone(),
            "function calls require an SL interpreter",
        ))
    }
}

// Expression evaluation

pub fn eval(context: &mut Context, exp: &Exp) -> Result<ValueRef, InterpError> {
    eval_with_calls(context, &mut RejectCalls, exp)
}

pub(crate) fn eval_with_calls(
    context: &mut Context,
    calls: &mut dyn Calls,
    exp: &Exp,
) -> Result<ValueRef, InterpError> {
    match &exp.kind {
        ExpKind::BoolE(value) => Ok(make::bool(*value, Region::none())),
        ExpKind::NumE(value) => Ok(make::num(value.clone(), Region::none())),
        ExpKind::TextE(value) => Ok(make::text(value.clone(), Region::none())),
        ExpKind::VarE(id) => context
            .find_value(&Variable::new(id.clone(), Vec::new()))
            .map(Rc::clone),
        ExpKind::UnE(operator, _operator_type, operand) => {
            eval_unary(context, calls, exp, *operator, operand)
        }
        ExpKind::BinE(operator, _operator_type, left, right) => {
            eval_binary(context, calls, exp, *operator, left, right)
        }
        ExpKind::CmpE(operator, _operator_type, left, right) => {
            eval_comparison(context, calls, exp, *operator, left, right)
        }
        ExpKind::UpCastE(typ, value) => {
            let value = eval_with_calls(context, calls, value)?;
            cast_up(context, typ, value)
        }
        ExpKind::DownCastE(typ, value) => {
            let value = eval_with_calls(context, calls, value)?;
            cast_down(context, typ, value)
        }
        ExpKind::SubE(value, typ) => {
            let value = eval_with_calls(context, calls, value)?;
            Ok(make::bool(
                calls.value_is_subtype(context, typ, &value)?,
                Region::none(),
            ))
        }
        ExpKind::MatchE(value, pattern) => eval_match(context, calls, exp, value, pattern),
        ExpKind::TupleE(exps) => eval_tuple(context, calls, exp, exps),
        ExpKind::CaseE(not_exp) => eval_case(context, calls, exp, not_exp),
        ExpKind::StrE(fields) => eval_structure(context, calls, exp, fields),
        ExpKind::OptE(value) => eval_option(context, calls, exp, value.as_deref()),
        ExpKind::ListE(exps) => eval_list(context, calls, exp, exps),
        ExpKind::ConsE(head, tail) => eval_cons(context, calls, exp, head, tail),
        ExpKind::CatE(left, right) => eval_concatenation(context, calls, exp, left, right),
        ExpKind::MemE(element, collection) => {
            eval_membership(context, calls, exp, element, collection)
        }
        ExpKind::LenE(value) => eval_length(context, calls, exp, value),
        ExpKind::DotE(base, atom) => eval_dot(context, calls, exp, base, atom),
        ExpKind::IdxE(base, index) => eval_index(context, calls, exp, base, index),
        ExpKind::SliceE(base, index, count) => eval_slice(context, calls, exp, base, index, count),
        ExpKind::UpdE(base, path, replacement) => {
            eval_update(context, calls, base, path, replacement)
        }
        ExpKind::CallE(id, type_args, args) => eval_call(context, calls, id, type_args, args),
        ExpKind::IterE(inner, iter_exp) => eval_iteration(context, calls, exp, inner, iter_exp),
    }
}

fn type_note(exp: &Exp) -> Typ {
    crate::domain::source::Spanned::new(exp.ty.clone(), exp.span.clone())
}

fn path_type_note(path: &Path) -> Typ {
    crate::domain::source::Spanned::new(path.ty.clone(), path.span.clone())
}

fn eval_all(
    context: &mut Context,
    calls: &mut dyn Calls,
    exps: &[Exp],
) -> Result<Vec<ValueRef>, InterpError> {
    exps.iter()
        .map(|exp| eval_with_calls(context, calls, exp))
        .collect()
}

// Unary expression evaluation

fn eval_unary(
    context: &mut Context,
    calls: &mut dyn Calls,
    outer: &Exp,
    operator: UnOp,
    operand: &Exp,
) -> Result<ValueRef, InterpError> {
    let value = eval_with_calls(context, calls, operand)?;
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
    context: &mut Context,
    calls: &mut dyn Calls,
    outer: &Exp,
    operator: BinOp,
    left: &Exp,
    right: &Exp,
) -> Result<ValueRef, InterpError> {
    // OCaml evaluates both operands before dispatching the operator.
    let value_left = eval_with_calls(context, calls, left)?;
    let value_right = eval_with_calls(context, calls, right)?;
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
    context: &mut Context,
    calls: &mut dyn Calls,
    outer: &Exp,
    operator: CmpOp,
    left: &Exp,
    right: &Exp,
) -> Result<ValueRef, InterpError> {
    let value_left = eval_with_calls(context, calls, left)?;
    let value_right = eval_with_calls(context, calls, right)?;
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

// Upcast expression evaluation

fn cast_error(typ: &Typ, direction: &str) -> InterpError {
    InterpError::new(
        typ.span.clone(),
        format!("cannot {direction} value to the requested type"),
    )
}

fn alias_target(context: &Context, typ: &Typ) -> Result<Option<Typ>, InterpError> {
    let TypKind::VarT(type_id, type_args) = &typ.node else {
        return Ok(None);
    };
    let type_def = context.find_type_def(type_id)?;
    let TypeDef::Defined(type_params, def_type) = type_def else {
        return Ok(None);
    };
    let DefTypKind::PlainT(inner) = &def_type.node else {
        return Ok(None);
    };
    if type_params.len() != type_args.len() {
        return Err(InterpError::new(
            typ.span.clone(),
            "type argument arity mismatch",
        ));
    }
    let substitution: TypeSubstitution = type_params
        .iter()
        .zip(type_args)
        .map(|(param, arg)| (param.node.clone(), arg.clone()))
        .collect();
    subst::subst_type(&substitution, inner)
        .map(Some)
        .map_err(|error| InterpError::new(typ.span.clone(), error.to_string()))
}

fn cast_up(context: &Context, typ: &Typ, value: ValueRef) -> Result<ValueRef, InterpError> {
    match &typ.node {
        TypKind::NumT(num::Typ::IntT) => match &value.kind {
            ValueKind::NumV(num::T::Nat(value)) => Ok(make::int(value.clone(), Region::none())),
            ValueKind::NumV(num::T::Int(_)) => Ok(value),
            _ => Err(cast_error(typ, "upcast")),
        },
        TypKind::VarT(..) => match alias_target(context, typ)? {
            Some(inner) => cast_up(context, &inner, value),
            None => Ok(value),
        },
        TypKind::TupleT(types) => {
            let values = get::tuple(&value).map_err(|_| cast_error(typ, "upcast"))?;
            if types.len() != values.len() {
                return Err(cast_error(typ, "upcast"));
            }
            let values = types
                .iter()
                .zip(values)
                .map(|(typ, value)| cast_up(context, typ, Rc::clone(value)))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(make::tuple(typ, values, Region::none()))
        }
        TypKind::IterT(inner, Iter::Opt) => {
            let value = get::opt(&value).map_err(|_| cast_error(typ, "upcast"))?;
            let value = value
                .map(|value| cast_up(context, inner, Rc::clone(value)))
                .transpose()?;
            Ok(make::opt(inner, value, Region::none()))
        }
        TypKind::IterT(inner, Iter::List) => {
            let values = get::list(&value).map_err(|_| cast_error(typ, "upcast"))?;
            let values = values
                .iter()
                .map(|value| cast_up(context, inner, Rc::clone(value)))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(make::list(inner, values, Region::none()))
        }
        _ => Ok(value),
    }
}

// Downcast expression evaluation

fn cast_down(context: &Context, typ: &Typ, value: ValueRef) -> Result<ValueRef, InterpError> {
    match &typ.node {
        TypKind::NumT(num::Typ::NatT) => match &value.kind {
            ValueKind::NumV(num::T::Nat(_)) => Ok(value),
            ValueKind::NumV(num::T::Int(value)) if value >= &BigInt::zero() => {
                Ok(make::nat(value.clone(), Region::none()))
            }
            _ => Err(cast_error(typ, "downcast")),
        },
        TypKind::VarT(..) => match alias_target(context, typ)? {
            Some(inner) => cast_down(context, &inner, value),
            None => Ok(value),
        },
        TypKind::TupleT(types) => {
            let values = get::tuple(&value).map_err(|_| cast_error(typ, "downcast"))?;
            if types.len() != values.len() {
                return Err(cast_error(typ, "downcast"));
            }
            let values = types
                .iter()
                .zip(values)
                .map(|(typ, value)| cast_down(context, typ, Rc::clone(value)))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(make::tuple(typ, values, Region::none()))
        }
        TypKind::IterT(inner, Iter::Opt) => {
            let value = get::opt(&value).map_err(|_| cast_error(typ, "downcast"))?;
            let value = value
                .map(|value| cast_down(context, inner, Rc::clone(value)))
                .transpose()?;
            Ok(make::opt(inner, value, Region::none()))
        }
        TypKind::IterT(inner, Iter::List) => {
            let values = get::list(&value).map_err(|_| cast_error(typ, "downcast"))?;
            let values = values
                .iter()
                .map(|value| cast_down(context, inner, Rc::clone(value)))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(make::list(inner, values, Region::none()))
        }
        _ => Ok(value),
    }
}

// Subtype check expression evaluation

fn value_is_subtype_uncached(
    context: &Context,
    typ: &Typ,
    value: &Value,
) -> Result<bool, InterpError> {
    match &typ.node {
        TypKind::BoolT => Ok(matches!(value.kind, ValueKind::BoolV(_))),
        TypKind::NumT(num::Typ::NatT) => Ok(match &value.kind {
            ValueKind::NumV(num::T::Nat(_)) => true,
            ValueKind::NumV(num::T::Int(value)) => value >= &BigInt::zero(),
            _ => false,
        }),
        TypKind::NumT(num::Typ::IntT) => Ok(matches!(value.kind, ValueKind::NumV(_))),
        TypKind::TextT => Ok(matches!(value.kind, ValueKind::TextV(_))),
        TypKind::VarT(type_id, type_args) => match context.find_type_def(type_id)? {
            TypeDef::Param | TypeDef::Defining(_) => Err(InterpError::new(
                typ.span.clone(),
                "unexpected type variable",
            )),
            TypeDef::Extern => Ok(matches!(value.kind, ValueKind::ExternV(_))),
            TypeDef::Defined(type_params, def_type) => {
                if type_params.len() != type_args.len() {
                    return Err(InterpError::new(
                        typ.span.clone(),
                        "type argument arity mismatch",
                    ));
                }
                let substitution: TypeSubstitution = type_params
                    .iter()
                    .zip(type_args)
                    .map(|(param, arg)| (param.node.clone(), arg.clone()))
                    .collect();
                match (&def_type.node, &value.kind) {
                    (DefTypKind::PlainT(inner), _) => {
                        let inner = subst::subst_type(&substitution, inner).map_err(|error| {
                            InterpError::new(typ.span.clone(), error.to_string())
                        })?;
                        value_is_subtype_uncached(context, &inner, value)
                    }
                    (DefTypKind::StructT(type_fields), ValueKind::StructV(value_fields)) => {
                        if type_fields.len() != value_fields.len() {
                            return Ok(false);
                        }
                        for ((type_atom, field_type), (value_atom, field_value)) in
                            type_fields.iter().zip(value_fields)
                        {
                            if type_atom.node != value_atom.node {
                                return Ok(false);
                            }
                            let field_type =
                                subst::subst_type(&substitution, field_type).map_err(|error| {
                                    InterpError::new(typ.span.clone(), error.to_string())
                                })?;
                            if !value_is_subtype_uncached(context, &field_type, field_value)? {
                                return Ok(false);
                            }
                        }
                        Ok(true)
                    }
                    (DefTypKind::VariantT(type_cases), ValueKind::CaseV(value_case)) => {
                        for (not_type, _, _) in type_cases {
                            if !not_type.node.same_shape(value_case) {
                                continue;
                            }
                            let not_type = subst::subst_not_type(&substitution, not_type).map_err(
                                |error| InterpError::new(typ.span.clone(), error.to_string()),
                            )?;
                            let types = not_type.node.args();
                            let values = value_case.args();
                            if types.len() != values.len() {
                                continue;
                            }
                            let mut matches = true;
                            for (typ, value) in types.into_iter().zip(values) {
                                if !value_is_subtype_uncached(context, typ, value)? {
                                    matches = false;
                                    break;
                                }
                            }
                            if matches {
                                return Ok(true);
                            }
                        }
                        Ok(false)
                    }
                    _ => Ok(false),
                }
            }
        },
        TypKind::TupleT(types) => match &value.kind {
            ValueKind::TupleV(values) if types.len() == values.len() => {
                for (typ, value) in types.iter().zip(values) {
                    if !value_is_subtype_uncached(context, typ, value)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        },
        TypKind::IterT(inner, Iter::Opt) => match &value.kind {
            ValueKind::OptV(Some(value)) => value_is_subtype_uncached(context, inner, value),
            ValueKind::OptV(None) => Ok(true),
            // Preserve the authoritative OCaml behavior.
            _ => Ok(true),
        },
        TypKind::IterT(inner, Iter::List) => match &value.kind {
            ValueKind::ListV(values) => {
                for value in values {
                    if !value_is_subtype_uncached(context, inner, value)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        },
        TypKind::FuncT(..) => Ok(false),
    }
}

// Caching subtyping of type variables to values,
// using semantic runtime values because Rust values intentionally omit generated ids.
pub(super) fn value_is_subtype(
    cache: &mut value_match::SubCache,
    context: &Context,
    typ: &Typ,
    value: &ValueRef,
) -> Result<bool, InterpError> {
    match &typ.node {
        TypKind::VarT(type_id, type_args) if type_args.is_empty() => {
            let key = (type_id.node.clone(), Rc::clone(value));
            if let Some(result) = cache.get(&key) {
                return Ok(*result);
            }
            let result = value_is_subtype_uncached(context, typ, value)?;
            cache.insert(key, result);
            Ok(result)
        }
        _ => value_is_subtype_uncached(context, typ, value),
    }
}

// Pattern match check expression evaluation

fn eval_match(
    context: &mut Context,
    calls: &mut dyn Calls,
    _outer: &Exp,
    exp: &Exp,
    pattern: &Pattern,
) -> Result<ValueRef, InterpError> {
    let value = eval_with_calls(context, calls, exp)?;
    let matches = match pattern {
        Pattern::CaseP(mixop) => get::case(&value)
            .map(|value_case| value_case.split().0 == *mixop)
            .unwrap_or(false),
        Pattern::ListP(pattern) => get::list(&value)
            .map(|values| match pattern {
                ListPattern::Cons => !values.is_empty(),
                ListPattern::Fixed(len) => usize::try_from(*len) == Ok(values.len()),
                ListPattern::Nil => values.is_empty(),
            })
            .unwrap_or(false),
        Pattern::OptP(pattern) => get::opt(&value)
            .map(|value| match pattern {
                OptPattern::Some => value.is_some(),
                OptPattern::None => value.is_none(),
            })
            .unwrap_or(false),
    };
    Ok(make::bool(matches, Region::none()))
}

// Tuple expression evaluation

fn eval_tuple(
    context: &mut Context,
    calls: &mut dyn Calls,
    outer: &Exp,
    exps: &[Exp],
) -> Result<ValueRef, InterpError> {
    let values = eval_all(context, calls, exps)?;
    Ok(make::tuple(&type_note(outer), values, Region::none()))
}

// Case expression evaluation

fn eval_case(
    context: &mut Context,
    calls: &mut dyn Calls,
    outer: &Exp,
    not_exp: &crate::lang::il::ast::NotExp,
) -> Result<ValueRef, InterpError> {
    let (mixop, exps) = not_exp.split();
    let values = exps
        .into_iter()
        .map(|exp| eval_with_calls(context, calls, exp))
        .collect::<Result<Vec<_>, _>>()?;
    let value_case = crate::domain::mixfix::Mixop::fill(&mixop, values)
        .expect("the mixop came from the same case expression");
    Ok(make::case(&type_note(outer), value_case, Region::none()))
}

// Struct expression evaluation

fn eval_structure(
    context: &mut Context,
    calls: &mut dyn Calls,
    outer: &Exp,
    fields: &[(crate::lang::il::ast::Atom, Exp)],
) -> Result<ValueRef, InterpError> {
    let value_fields = fields
        .iter()
        .map(|(atom, exp)| eval_with_calls(context, calls, exp).map(|value| (atom.clone(), value)))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(make::structure(
        &type_note(outer),
        value_fields,
        Region::none(),
    ))
}

// Option expression evaluation

fn eval_option(
    context: &mut Context,
    calls: &mut dyn Calls,
    outer: &Exp,
    exp: Option<&Exp>,
) -> Result<ValueRef, InterpError> {
    let value = exp
        .map(|exp| eval_with_calls(context, calls, exp))
        .transpose()?;
    Ok(make::opt(&type_note(outer), value, Region::none()))
}

// List expression evaluation

fn eval_list(
    context: &mut Context,
    calls: &mut dyn Calls,
    outer: &Exp,
    exps: &[Exp],
) -> Result<ValueRef, InterpError> {
    let values = eval_all(context, calls, exps)?;
    Ok(make::list(&type_note(outer), values, Region::none()))
}

// Cons expression evaluation

fn eval_cons(
    context: &mut Context,
    calls: &mut dyn Calls,
    outer: &Exp,
    head: &Exp,
    tail: &Exp,
) -> Result<ValueRef, InterpError> {
    let value_head = eval_with_calls(context, calls, head)?;
    let value_tail = eval_with_calls(context, calls, tail)?;
    let mut values = Vec::with_capacity(
        get::list(&value_tail)
            .map_err(|error| value_error(outer, error.to_string()))?
            .len()
            + 1,
    );
    values.push(value_head);
    values.extend(
        get::list(&value_tail)
            .expect("the tail was checked")
            .iter()
            .cloned(),
    );
    Ok(make::list(&type_note(outer), values, Region::none()))
}

// Concatenation expression evaluation

fn eval_concatenation(
    context: &mut Context,
    calls: &mut dyn Calls,
    outer: &Exp,
    left: &Exp,
    right: &Exp,
) -> Result<ValueRef, InterpError> {
    let value_left = eval_with_calls(context, calls, left)?;
    let value_right = eval_with_calls(context, calls, right)?;
    match (&value_left.kind, &value_right.kind) {
        (
            crate::runtime::value::ValueKind::TextV(left),
            crate::runtime::value::ValueKind::TextV(right),
        ) => Ok(make::text(format!("{left}{right}"), Region::none())),
        (
            crate::runtime::value::ValueKind::ListV(left),
            crate::runtime::value::ValueKind::ListV(right),
        ) => Ok(make::list(
            &type_note(outer),
            left.iter().chain(right).cloned().collect(),
            Region::none(),
        )),
        _ => Err(value_error(
            outer,
            "concatenation expects either two texts or two lists",
        )),
    }
}

// Membership expression evaluation

fn eval_membership(
    context: &mut Context,
    calls: &mut dyn Calls,
    outer: &Exp,
    element: &Exp,
    collection: &Exp,
) -> Result<ValueRef, InterpError> {
    let value_element = eval_with_calls(context, calls, element)?;
    let value_collection = eval_with_calls(context, calls, collection)?;
    let values =
        get::list(&value_collection).map_err(|error| value_error(outer, error.to_string()))?;
    Ok(make::bool(values.contains(&value_element), Region::none()))
}

// Length expression evaluation

fn eval_length(
    context: &mut Context,
    calls: &mut dyn Calls,
    outer: &Exp,
    exp: &Exp,
) -> Result<ValueRef, InterpError> {
    let value = eval_with_calls(context, calls, exp)?;
    let len = match &value.kind {
        crate::runtime::value::ValueKind::TextV(value) => value.len(),
        crate::runtime::value::ValueKind::ListV(values) => values.len(),
        _ => {
            return Err(value_error(
                outer,
                "length operation expects either a text or a list",
            ));
        }
    };
    Ok(make::nat(BigInt::from(len), Region::none()))
}

// Dot expression evaluation

fn eval_dot(
    context: &mut Context,
    calls: &mut dyn Calls,
    outer: &Exp,
    base: &Exp,
    atom: &crate::lang::il::ast::Atom,
) -> Result<ValueRef, InterpError> {
    let value_base = eval_with_calls(context, calls, base)?;
    let fields =
        get::structure(&value_base).map_err(|error| value_error(outer, error.to_string()))?;
    fields
        .iter()
        .find(|(field, _)| field.node == atom.node)
        .map(|(_, value)| Rc::clone(value))
        .ok_or_else(|| value_error(outer, "structure field is undefined"))
}

fn index_of_value(outer: &Exp, value: &Value) -> Result<isize, InterpError> {
    let index = match num_of_value(outer, value)? {
        num::T::Nat(value) | num::T::Int(value) => value,
    };
    index
        .to_isize()
        .ok_or_else(|| value_error(outer, "index is too large"))
}

// Index expression evaluation

fn eval_index(
    context: &mut Context,
    calls: &mut dyn Calls,
    outer: &Exp,
    base: &Exp,
    index: &Exp,
) -> Result<ValueRef, InterpError> {
    let value_base = eval_with_calls(context, calls, base)?;
    let value_index = eval_with_calls(context, calls, index)?;
    let index = index_of_value(outer, &value_index)?;
    match &value_base.kind {
        crate::runtime::value::ValueKind::TextV(value) => {
            let index = bounded_index(outer, index, value.len())?;
            let byte = value
                .as_bytes()
                .get(index)
                .copied()
                .expect("the index was checked");
            let text = str::from_utf8(std::slice::from_ref(&byte))
                .map_err(|_| value_error(outer, "indexed byte is not valid UTF-8"))?;
            Ok(make::text(text.to_owned(), Region::none()))
        }
        crate::runtime::value::ValueKind::ListV(values) => {
            let index = bounded_index(outer, index, values.len())?;
            Ok(Rc::clone(&values[index]))
        }
        _ => Err(value_error(
            outer,
            "indexing expects either a text or a list",
        )),
    }
}

fn bounded_index(outer: &Exp, index: isize, len: usize) -> Result<usize, InterpError> {
    let Ok(index) = usize::try_from(index) else {
        return Err(value_error(outer, "index out of bounds"));
    };
    if index >= len {
        Err(value_error(outer, "index out of bounds"))
    } else {
        Ok(index)
    }
}

// Slice expression evaluation

fn eval_slice(
    context: &mut Context,
    calls: &mut dyn Calls,
    outer: &Exp,
    base: &Exp,
    index: &Exp,
    count: &Exp,
) -> Result<ValueRef, InterpError> {
    let value_base = eval_with_calls(context, calls, base)?;
    let value_index = eval_with_calls(context, calls, index)?;
    let index = index_of_value(outer, &value_index)?;
    let value_count = eval_with_calls(context, calls, count)?;
    let count = index_of_value(outer, &value_count)?;
    if index < 0 || count < 0 {
        return Err(value_error(outer, "slice out of bounds"));
    }
    let start = usize::try_from(index).expect("non-negative index");
    let count = usize::try_from(count).expect("non-negative count");
    let end = start
        .checked_add(count)
        .ok_or_else(|| value_error(outer, "slice out of bounds"))?;
    match &value_base.kind {
        crate::runtime::value::ValueKind::TextV(value) => {
            if end > value.len() {
                return Err(value_error(outer, "slice out of bounds"));
            }
            let text = value
                .get(start..end)
                .ok_or_else(|| value_error(outer, "slice is not valid UTF-8"))?;
            Ok(make::text(text.to_owned(), Region::none()))
        }
        crate::runtime::value::ValueKind::ListV(values) => {
            if end > values.len() {
                return Err(value_error(outer, "slice out of bounds"));
            }
            Ok(make::list(
                &type_note(outer),
                values[start..end].to_vec(),
                Region::none(),
            ))
        }
        _ => Err(value_error(
            outer,
            "slicing expects either a text or a list",
        )),
    }
}

// Update expression evaluation

fn eval_access_path(
    context: &mut Context,
    calls: &mut dyn Calls,
    value_base: &ValueRef,
    path: &Path,
) -> Result<ValueRef, InterpError> {
    match &path.kind {
        PathKind::RootP => Ok(Rc::clone(value_base)),
        PathKind::IdxP(inner, index) => {
            let value = eval_access_path(context, calls, value_base, inner)?;
            let value_index = eval_with_calls(context, calls, index)?;
            let index_value = index_of_value(index, &value_index)?;
            match &value.kind {
                crate::runtime::value::ValueKind::TextV(text) => {
                    let index_value = bounded_index(index, index_value, text.len())?;
                    let byte = text
                        .as_bytes()
                        .get(index_value)
                        .copied()
                        .expect("the index was checked");
                    let text = str::from_utf8(std::slice::from_ref(&byte))
                        .map_err(|_| path_error(path, "indexed byte is not valid UTF-8"))?;
                    Ok(make::text(text.to_owned(), Region::none()))
                }
                crate::runtime::value::ValueKind::ListV(values) => {
                    let index_value = bounded_index(index, index_value, values.len())?;
                    Ok(Rc::clone(&values[index_value]))
                }
                _ => Err(InterpError::new(
                    path.span.clone(),
                    "indexing expects either a text or a list",
                )),
            }
        }
        PathKind::SliceP(inner, index, count) => {
            let value = eval_access_path(context, calls, value_base, inner)?;
            let (start, end) = eval_path_slice_bounds(context, calls, index, count)?;
            match &value.kind {
                crate::runtime::value::ValueKind::TextV(text) => {
                    if end > text.len() {
                        return Err(value_error(count, "slice out of bounds"));
                    }
                    let text = text
                        .get(start..end)
                        .ok_or_else(|| path_error(path, "slice is not valid UTF-8"))?;
                    Ok(make::text(text.to_owned(), Region::none()))
                }
                crate::runtime::value::ValueKind::ListV(values) => {
                    if end > values.len() {
                        return Err(value_error(count, "slice out of bounds"));
                    }
                    Ok(make::list(
                        &path_type_note(inner),
                        values[start..end].to_vec(),
                        Region::none(),
                    ))
                }
                _ => Err(InterpError::new(
                    path.span.clone(),
                    "slicing expects either a text or a list",
                )),
            }
        }
        PathKind::DotP(inner, atom) => {
            let value = eval_access_path(context, calls, value_base, inner)?;
            let fields = get::structure(&value)
                .map_err(|error| InterpError::new(path.span.clone(), error.to_string()))?;
            fields
                .iter()
                .find(|(field, _)| field.node == atom.node)
                .map(|(_, value)| Rc::clone(value))
                .ok_or_else(|| InterpError::new(path.span.clone(), "structure field is undefined"))
        }
    }
}

fn eval_path_slice_bounds(
    context: &mut Context,
    calls: &mut dyn Calls,
    index: &Exp,
    count: &Exp,
) -> Result<(usize, usize), InterpError> {
    let value_index = eval_with_calls(context, calls, index)?;
    let index_value = index_of_value(index, &value_index)?;
    let value_count = eval_with_calls(context, calls, count)?;
    let count_value = index_of_value(count, &value_count)?;
    if index_value < 0 || count_value < 0 {
        return Err(value_error(count, "slice out of bounds"));
    }
    let start = usize::try_from(index_value).expect("non-negative index");
    let count_value = usize::try_from(count_value).expect("non-negative count");
    let end = start
        .checked_add(count_value)
        .ok_or_else(|| value_error(count, "slice out of bounds"))?;
    Ok((start, end))
}

fn eval_update_path(
    context: &mut Context,
    calls: &mut dyn Calls,
    value_base: &ValueRef,
    path: &Path,
    value_update: ValueRef,
) -> Result<ValueRef, InterpError> {
    match &path.kind {
        PathKind::RootP => Ok(value_update),
        PathKind::IdxP(inner, index) => {
            let value = eval_access_path(context, calls, value_base, inner)?;
            let value_index = eval_with_calls(context, calls, index)?;
            let index_value = index_of_value(index, &value_index)?;
            let value = match &value.kind {
                crate::runtime::value::ValueKind::TextV(text) => {
                    let index_value = bounded_index(index, index_value, text.len())?;
                    let replacement = get::text(&value_update)
                        .map_err(|error| value_error(index, error.to_string()))?;
                    if replacement.len() != 1 {
                        return Err(value_error(
                            index,
                            "updating a character requires a single-character text",
                        ));
                    }
                    let mut bytes = text.as_bytes().to_vec();
                    bytes[index_value] = replacement.as_bytes()[0];
                    let text = String::from_utf8(bytes)
                        .map_err(|_| path_error(path, "updated text is not valid UTF-8"))?;
                    make::text(text, Region::none())
                }
                crate::runtime::value::ValueKind::ListV(values) => {
                    let index_value = bounded_index(index, index_value, values.len())?;
                    let mut values = values.clone();
                    values[index_value] = value_update;
                    make::list(&path_type_note(inner), values, Region::none())
                }
                _ => {
                    return Err(InterpError::new(
                        path.span.clone(),
                        "indexing expects either a text or a list",
                    ));
                }
            };
            eval_update_path(context, calls, value_base, inner, value)
        }
        PathKind::SliceP(inner, index, count) => {
            let value = eval_access_path(context, calls, value_base, inner)?;
            let (start, end) = eval_path_slice_bounds(context, calls, index, count)?;
            let update_len = end - start;
            let value = match &value.kind {
                crate::runtime::value::ValueKind::TextV(text) => {
                    if end > text.len() {
                        return Err(value_error(count, "slice out of bounds"));
                    }
                    let replacement = get::text(&value_update)
                        .map_err(|error| value_error(count, error.to_string()))?;
                    if replacement.len() != update_len {
                        return Err(value_error(
                            count,
                            format!(
                                "updating a slice of length {update_len} requires a text of length {}",
                                replacement.len()
                            ),
                        ));
                    }
                    let mut bytes = text.as_bytes().to_vec();
                    bytes[start..end].copy_from_slice(replacement.as_bytes());
                    let text = String::from_utf8(bytes)
                        .map_err(|_| path_error(path, "updated text is not valid UTF-8"))?;
                    make::text(text, Region::none())
                }
                crate::runtime::value::ValueKind::ListV(values) => {
                    if end > values.len() {
                        return Err(value_error(count, "slice out of bounds"));
                    }
                    let replacements = get::list(&value_update)
                        .map_err(|error| value_error(count, error.to_string()))?;
                    if replacements.len() != update_len {
                        return Err(value_error(
                            count,
                            format!(
                                "updating a slice of length {update_len} requires a list of length {}",
                                replacements.len()
                            ),
                        ));
                    }
                    let mut values = values.clone();
                    values[start..end].clone_from_slice(replacements);
                    make::list(&path_type_note(inner), values, Region::none())
                }
                _ => {
                    return Err(InterpError::new(
                        path.span.clone(),
                        "slicing expects either a text or a list",
                    ));
                }
            };
            eval_update_path(context, calls, value_base, inner, value)
        }
        PathKind::DotP(inner, atom) => {
            let value = eval_access_path(context, calls, value_base, inner)?;
            let fields = get::structure(&value)
                .map_err(|error| InterpError::new(path.span.clone(), error.to_string()))?;
            let fields = fields
                .iter()
                .map(|(field, value)| {
                    if field.node == atom.node {
                        (field.clone(), Rc::clone(&value_update))
                    } else {
                        (field.clone(), Rc::clone(value))
                    }
                })
                .collect();
            let value = make::structure(&path_type_note(inner), fields, Region::none());
            eval_update_path(context, calls, value_base, inner, value)
        }
    }
}

fn eval_update(
    context: &mut Context,
    calls: &mut dyn Calls,
    base: &Exp,
    path: &Path,
    replacement: &Exp,
) -> Result<ValueRef, InterpError> {
    let value_base = eval_with_calls(context, calls, base)?;
    let value_replacement = eval_with_calls(context, calls, replacement)?;
    eval_update_path(context, calls, &value_base, path, value_replacement)
}

// Function call expression evaluation

fn resolve_type_args(context: &Context, type_args: &[Typ]) -> Result<Vec<Typ>, InterpError> {
    subst::subst_types(&context.local_type_substitution(), type_args).map_err(
        |error| match &error {
            subst::SubstError::HigherOrder { span } => {
                InterpError::new(span.clone(), error.to_string())
            }
        },
    )
}

fn eval_call(
    context: &mut Context,
    calls: &mut dyn Calls,
    id: &crate::lang::il::ast::Id,
    type_args: &[Typ],
    args: &[Arg],
) -> Result<ValueRef, InterpError> {
    let (type_args, values) = eval_call_inputs(context, calls, type_args, args)?;
    calls.invoke_func(context, id, &type_args, &values)
}

pub(crate) fn eval_call_inputs(
    context: &mut Context,
    calls: &mut dyn Calls,
    type_args: &[Typ],
    args: &[Arg],
) -> Result<(Vec<Typ>, Vec<ValueRef>), InterpError> {
    let type_args = resolve_type_args(context, type_args)?;
    let values = eval_args(context, calls, args)?;
    Ok((type_args, values))
}

// Iterated expression evaluation

fn eval_optional_iteration(
    context: &mut Context,
    calls: &mut dyn Calls,
    outer: &Exp,
    exp: &Exp,
    vars: &[crate::lang::il::ast::Var],
) -> Result<ValueRef, InterpError> {
    let value = match context.optional_bindings(vars)? {
        Some(bindings) => Some(
            context
                .with_value_bindings(bindings, |context| eval_with_calls(context, calls, exp))?,
        ),
        None => None,
    };
    Ok(make::opt(&type_note(outer), value, Region::none()))
}

fn eval_list_iteration(
    context: &mut Context,
    calls: &mut dyn Calls,
    outer: &Exp,
    exp: &Exp,
    vars: &[crate::lang::il::ast::Var],
) -> Result<ValueRef, InterpError> {
    let batches = context.list_binding_batches(vars)?;
    let mut values = Vec::with_capacity(batches.len());
    for bindings in batches {
        values.push(
            context
                .with_value_bindings(bindings, |context| eval_with_calls(context, calls, exp))?,
        );
    }
    Ok(make::list(&type_note(outer), values, Region::none()))
}

fn eval_iteration(
    context: &mut Context,
    calls: &mut dyn Calls,
    outer: &Exp,
    exp: &Exp,
    (iter, vars): &crate::lang::il::ast::IterExp,
) -> Result<ValueRef, InterpError> {
    if let Some(variable) = iterated_variable(outer) {
        return context.find_value(&variable).map(Rc::clone);
    }
    match iter {
        Iter::Opt => eval_optional_iteration(context, calls, outer, exp, vars),
        Iter::List => eval_list_iteration(context, calls, outer, exp, vars),
    }
}

// Argument evaluation

fn eval_arg(
    context: &mut Context,
    calls: &mut dyn Calls,
    arg: &Arg,
) -> Result<ValueRef, InterpError> {
    match &arg.node {
        ArgKind::ExpA(exp) => eval_with_calls(context, calls, exp),
        ArgKind::DefA(id) => {
            let signature = context.find_function(id)?.1.get_signature();
            Ok(make::func(
                id.clone(),
                signature.type_params,
                signature.param_types,
                signature.return_type,
                Region::none(),
            ))
        }
    }
}

fn eval_args(
    context: &mut Context,
    calls: &mut dyn Calls,
    args: &[Arg],
) -> Result<Vec<ValueRef>, InterpError> {
    args.iter()
        .map(|arg| eval_arg(context, calls, arg))
        .collect()
}
