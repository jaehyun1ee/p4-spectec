//! Destructuring assignments preserve iteration paths and isolate list rows
//!
//! `assign_exp` matches a value against a binder pattern
//! and binds its variables:
//! `(x, y) <- (1, 2)` binds `x` and `y`;
//! `x* <- [1, 2]` binds `x*` as a whole,
//! while `(x, y)* <- [...]` assigns each row in an isolated sub-context
//! and gathers the rows into `x*` and `y*`.

use std::{borrow::Borrow, rc::Rc};

use crate::lang::{
    common::source::Span,
    data::{
        value::{Arena, Value, ValueKind, get, make},
        var::IdSlot,
    },
    traits::at::At,
};

use crate::runtime::typdef::TypeDef;

use crate::interp::shared::{
    backtrack::{Backtrack, ok, unwrap, unwrap_from_result},
    prepare::ast,
    util::{find_slot_of_exp, iterate_vars},
};

use crate::phrase;

use super::super::context::{ReadContext, WriteContext};

// = Type parameter assignment

/// Binds type arguments in a local scope, requiring equal counts.
pub fn assign_tparams<Ctx: WriteContext>(
    mut ctx: Ctx,
    tparams: &[ast::TParam],
    targs: &[ast::Typ],
    span: &Span,
) -> Backtrack<Ctx> {
    // Check arity before binding any type parameter
    assert_eq!(tparams.len(), targs.len(), "type argument arity mismatch at {span}");
    // Type arguments shadow global definitions in the callee scope
    for (tparam, targ) in tparams.iter().zip(targs) {
        let def_typ = phrase!(node: ast::DefTypKind::Plain(targ.clone()), span: targ.span);
        unwrap_from_result!(
            ctx.add_typdef_local(tparam.clone(), TypeDef::Defined(vec![], Box::new(def_typ))),
            &tparam.span
        );
    }
    ok!(ctx)
}

// = Expression assignment

/// Matches `value` against the pattern `exp`, binding its variables.
pub fn assign_exp<Ctx: WriteContext>(
    arena: &mut Arena,
    mut ctx: Ctx,
    exp: &ast::Exp,
    value: Value,
) -> Backtrack<Ctx> {
    unwrap!(assign_exp_in(arena, &mut ctx, exp, value));
    ok!(ctx)
}

/// Binds recursively through one context borrowed by the owning entry point.
fn assign_exp_in<Ctx: WriteContext>(
    arena: &mut Arena,
    ctx: &mut Ctx,
    exp: &ast::Exp,
    value: Value,
) -> Backtrack<()> {
    match (&exp.node, arena.kind(&value)) {
        // A variable binds directly
        (ast::ExpKind::Id(id), _) => assign_id_exp(arena, ctx, id, value),
        // Tuple: componentwise
        (ast::ExpKind::Tuple(exps), ValueKind::Tuple(values)) => {
            let values_len = values.len();
            assign_children(arena, ctx, exps.iter(), value, values_len, |arena, value, idx| {
                get::tuple(arena, &value).expect("tuple assignment value")[idx]
            })
        }
        // Case: the arguments
        (ast::ExpKind::Case(not_exp), ValueKind::Case(value_case)) => {
            let values_len = value_case.args().len();
            assign_children(
                arena,
                ctx,
                not_exp.args().iter(),
                value,
                values_len,
                |arena, value, idx| {
                    get::case(arena, &value)
                        .expect("case assignment value")
                        .args()[idx]
                },
            )
        }
        // Struct: the fields in order
        (ast::ExpKind::Str(exp_fields), ValueKind::Struct(value_fields)) => {
            let values_len = value_fields.len();
            let exps = exp_fields.iter().map(|exp_field| &exp_field.exp);
            assign_children(arena, ctx, exps, value, values_len, |arena, value, idx| {
                get::structure(arena, &value).expect("struct assignment value")[idx].1
            })
        }
        // Option: both present or both absent
        (ast::ExpKind::Opt(exp_opt), ValueKind::Opt(value_opt)) => {
            let value_opt = *value_opt;
            assign_opt_exp(arena, ctx, exp_opt, &value_opt)
        }
        // List literal: elementwise
        (ast::ExpKind::List(exps), ValueKind::List(values)) => {
            let values_len = values.len();
            assign_children(arena, ctx, exps.iter(), value, values_len, |arena, value, idx| {
                get::list(arena, &value).expect("list assignment value")[idx]
            })
        }
        // Cons: the first element, then the rest
        (ast::ExpKind::Cons(exp_head, exp_tail), ValueKind::List(_)) => {
            assign_cons_exp(arena, ctx, exp, exp_head, exp_tail, &value)
        }
        // Iteration: as a whole or row by row
        (ast::ExpKind::Iter(exp_inner, exp_iter), _) => {
            assign_iter_exp(arena, ctx, exp, exp_inner, exp_iter, value)
        }
        // Lowering checks the pattern before destructuring it
        _ => unreachable!("assignment pattern must match the value"),
    }
}

/// Assigns values to patterns pairwise, requiring equal counts.
pub fn assign_exps<Ctx: WriteContext, T: Borrow<ast::Exp> + At>(
    arena: &mut Arena,
    mut ctx: Ctx,
    exps: &[T],
    values: &[Value],
) -> Backtrack<Ctx> {
    // Counts must match
    assert_eq!(exps.len(), values.len(), "assignment arity mismatch");
    for (exp, value) in exps.iter().zip(values) {
        unwrap!(assign_exp_in(arena, &mut ctx, exp.borrow(), *value));
    }
    ok!(ctx)
}

// - Identifier expression

fn assign_id_exp<Ctx: WriteContext>(
    _arena: &mut Arena,
    ctx: &mut Ctx,
    id: &IdSlot,
    value: Value,
) -> Backtrack<()> {
    ctx.add_value_at_slot(id.slot, value);
    ok!(())
}

// - Composite expressions

/// Reads each child again after recursion, which may grow the arena's storage.
fn assign_children<'a, Ctx: WriteContext>(
    arena: &mut Arena,
    ctx: &mut Ctx,
    exps: impl ExactSizeIterator<Item = &'a ast::Exp>,
    value: Value,
    values_len: usize,
    get_value: impl Fn(&Arena, Value, usize) -> Value,
) -> Backtrack<()> {
    // Check all counts before binding the first child
    assert_eq!(exps.len(), values_len, "assignment arity mismatch");
    // Parent bodies are immutable, but their storage may move during assignment
    for (idx, exp) in exps.enumerate() {
        let value = get_value(arena, value, idx);
        unwrap!(assign_exp_in(arena, ctx, exp, value));
    }
    ok!(())
}

// - Optional expression

/// Assigns an option: a payload to a payload, absence to absence.
fn assign_opt_exp<Ctx: WriteContext>(
    arena: &mut Arena,
    ctx: &mut Ctx,
    exp_opt: &Option<Box<ast::Exp>>,
    value_opt: &Option<Value>,
) -> Backtrack<()> {
    match (exp_opt, value_opt) {
        // Both present: assign the payload
        (Some(exp), Some(value)) => assign_exp_in(arena, ctx, exp, *value),
        // Both absent: nothing to bind
        (None, None) => ok!(()),
        // Lowering checks optionality before destructuring
        _ => unreachable!("option pattern must match the value"),
    }
}

// - Cons expression

/// Splits a non-empty list into head and tail and assigns each.
fn assign_cons_exp<Ctx: WriteContext>(
    arena: &mut Arena,
    ctx: &mut Ctx,
    exp: &ast::Exp,
    exp_head: &ast::Exp,
    exp_tail: &ast::Exp,
    value: &Value,
) -> Backtrack<()> {
    let values = get::list(arena, value).expect("cons assignment value must be a list");
    let (value_head, values_tail) = values
        .split_first()
        .expect("cons pattern must match a non-empty list");
    let value_head = *value_head;
    let values_tail = values_tail.to_vec();
    // Rebuild the tail as a list value of the same type
    let typ = phrase!(node: arena.typ(value).clone(), span: exp.span);
    let value_tail = unwrap_from_result!(
        make::list(arena, typ.node.clone(), values_tail, Span::default()),
        &Span::default()
    );
    unwrap!(assign_exp_in(arena, ctx, exp_head, value_head));
    assign_exp_in(arena, ctx, exp_tail, value_tail)
}

// - Iteration expression

/// Assigns an iterated pattern, binding its variables one iteration outward.
fn assign_iter_exp<Ctx: WriteContext>(
    arena: &mut Arena,
    ctx: &mut Ctx,
    exp: &ast::Exp,
    exp_inner: &ast::Exp,
    exp_iter: &ast::ExpIter,
    value: Value,
) -> Backtrack<()> {
    // A bare iterated variable binds as a whole
    if let Some(slot) = find_slot_of_exp(ctx, exp) {
        ctx.add_value_at_slot(slot, value);
        return ok!(());
    }
    // Otherwise assign each element in a sub-context and gather per variable
    let span = &exp.span;
    let vars_outer = iterate_vars(ctx, &exp_iter.vars, exp_iter.iter);
    match exp_iter.iter {
        // Option: assign the payload once, or bind every variable to none
        ast::Iter::Opt => {
            let value_opt =
                get::opt(arena, &value).expect("iteration assignment value must be an option");
            let ctx_sub = match value_opt {
                Some(value) => {
                    let mut ctx_sub = ctx.clone();
                    unwrap!(assign_exp_in(arena, &mut ctx_sub, exp_inner, value));
                    Some(ctx_sub)
                }
                None => None,
            };
            for (var, var_outer) in exp_iter.vars.iter().zip(&vars_outer) {
                let typ = var_outer.typ();
                let value_opt = ctx_sub.as_ref().map(|ctx_sub| {
                    *ctx_sub
                        .find_value_at_slot(var.slot)
                        .expect("value must be bound")
                });
                let value = unwrap_from_result!(
                    make::opt(arena, typ.node.into(), value_opt, Span::default()),
                    span
                );
                ctx.add_value_at_slot(var_outer.slot, value);
            }
            ok!(())
        }
        // List: reuse one sub-context, collecting only the iterated slots
        ast::Iter::List => {
            let values_len = get::list(arena, &value)
                .expect("iteration assignment value must be a list")
                .len();
            let mut ctx_sub = ctx.clone();
            ctx_sub.clear_value_bindings();
            let mut values_by_var: Vec<Vec<Option<Value>>> = exp_iter
                .vars
                .iter()
                .map(|_| Vec::with_capacity(values_len))
                .collect();
            for idx in 0..values_len {
                let value = get::list(arena, &value)
                    .expect("iteration assignment value must be a list")[idx];
                // A missing binding must not reuse a preceding row's value
                for var in &exp_iter.vars {
                    ctx_sub.remove_value_at_slot(var.slot);
                }
                unwrap!(assign_exp_in(arena, &mut ctx_sub, exp_inner, value));
                for (var, values) in exp_iter.vars.iter().zip(&mut values_by_var) {
                    values.push(ctx_sub.find_value_at_slot(var.slot).copied());
                }
            }
            // Each variable collects its per-row values into a list
            for (var_outer, values) in vars_outer.iter().zip(values_by_var) {
                let typ = var_outer.typ();
                // Check missing bindings in variable order, then row order
                let values = values
                    .into_iter()
                    .map(|value| value.expect("value must be bound"))
                    .collect();
                let value_sub = unwrap_from_result!(
                    make::list(arena, typ.node.into(), values, Span::default()),
                    span
                );
                ctx.add_value_at_slot(var_outer.slot, value_sub);
            }
            ok!(())
        }
    }
}

// = Argument assignment

/// Assigns an argument value: to a pattern, or as a function definition.
pub fn assign_arg<Ctx: WriteContext>(
    arena: &mut Arena,
    ctx_caller: &impl ReadContext<Func = Ctx::Func>,
    ctx_callee: Ctx,
    arg: &ast::Arg,
    value: Value,
) -> Backtrack<Ctx> {
    match &arg.node {
        ast::ArgKind::Exp(exp) => assign_exp_arg(arena, ctx_callee, exp, value),
        ast::ArgKind::Def(id) => assign_def(arena, ctx_caller, ctx_callee, id, value),
    }
}

/// Assigns values to arguments pairwise, requiring equal counts.
pub fn assign_args<Ctx: WriteContext>(
    arena: &mut Arena,
    ctx_caller: &impl ReadContext<Func = Ctx::Func>,
    ctx_callee: Ctx,
    args: &[ast::Arg],
    values: &[Value],
) -> Backtrack<Ctx> {
    // Counts must match
    assert_eq!(args.len(), values.len(), "validated argument assignment arity");
    let mut ctx = ctx_callee;
    for (arg, value) in args.iter().zip(values.iter()) {
        ctx = unwrap!(assign_arg(arena, ctx_caller, ctx, arg, *value));
    }
    ok!(ctx)
}

// - Expression argument

fn assign_exp_arg<Ctx: WriteContext>(
    arena: &mut Arena,
    ctx: Ctx,
    exp: &ast::Exp,
    value: Value,
) -> Backtrack<Ctx> {
    assign_exp(arena, ctx, exp, value)
}

// - Function argument

/// Binds a function argument in the callee from its definition in the caller.
pub fn assign_def<Ctx: WriteContext>(
    arena: &Arena,
    ctx_caller: &impl ReadContext<Func = Ctx::Func>,
    mut ctx_callee: Ctx,
    id: &ast::Id,
    value: Value,
) -> Backtrack<Ctx> {
    // The value must be a function reference
    let ValueKind::Func(id_func) = arena.kind(&value) else {
        unreachable!("function parameter must receive a function reference");
    };
    // Look the definition up in the caller, bind it in the callee
    let func = unwrap_from_result!(ctx_caller.find_func(id_func), &id_func.span);
    unwrap_from_result!(ctx_callee.add_func(id.clone(), Rc::clone(func)), &id.span);
    ok!(ctx_callee)
}
