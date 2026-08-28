//! Elaboration-language validation and conversion to intermediate syntax

use crate::{
    lang::{
        common::{
            Id,
            ds::map::ArityMismatch,
            notation::mixfix::Mixfix,
            noted::Noted,
            source::{Span, Spanned},
        },
        el::ast as el,
        hints::input,
        il::{ast as il, fresh as il_fresh, var as il_var},
        traits::free::Free,
        xl,
    },
    runtime::types::{
        Theta, TypeArityMismatch, TypeDef, TypeError, TypeErrorKind, equiv_func_typ, equiv_typ,
        expand_typ, optimize_sub_typ, sub_typ, subst_params, subst_typ, subst_typ_case,
    },
};

use super::{
    ElabError, ElabErrorKind, EntityKind, TypeShape, attempt::Attempt, context::Context, dimension,
};

macro_rules! attempt {
    ($attempt:expr) => {
        match $attempt {
            Attempt::Ok(value) => value,
            Attempt::Fail(traces) => return Attempt::Fail(traces),
        }
    };
}

fn type_result<T>(result: Result<T, TypeError>) -> Attempt<T> {
    match result {
        Ok(value) => Attempt::ok(value),
        Err(error) => Attempt::fail(error.into()),
    }
}

fn elab_iter(iter: el::Iter) -> il::Iter {
    match iter {
        el::Iter::Opt => il::Iter::Opt,
        el::Iter::List => il::Iter::List,
    }
}

fn destruct_error(shape: TypeShape, span: Span) -> ElabError {
    ElabError::new(
        ElabErrorKind::CannotDestructure(shape),
        span,
        format!("cannot destruct type as {shape}"),
    )
}

fn as_text_typ(ctx: &Context, typ: &il::Typ) -> Attempt<()> {
    type_result(expand_typ(&ctx.tdenv, typ)).and_then(|typ| match typ.node {
        il::TypKind::Text => Attempt::ok(()),
        _ => Attempt::fail(destruct_error(TypeShape::Text, typ.span)),
    })
}

fn as_iter_typ(ctx: &Context, typ: &il::Typ) -> Attempt<(il::Typ, il::Iter)> {
    type_result(expand_typ(&ctx.tdenv, typ)).and_then(|typ| match typ.node {
        il::TypKind::Iter(typ, iter) => Attempt::ok((*typ, iter)),
        _ => Attempt::fail(destruct_error(TypeShape::Iteration, typ.span)),
    })
}

fn as_tuple_typ(ctx: &Context, typ: &il::Typ) -> Attempt<Vec<il::Typ>> {
    type_result(expand_typ(&ctx.tdenv, typ)).and_then(|typ| match typ.node {
        il::TypKind::Tuple(typs) => Attempt::ok(typs),
        _ => Attempt::fail(destruct_error(TypeShape::Tuple, typ.span)),
    })
}

fn as_list_typ(ctx: &Context, typ: &il::Typ) -> Attempt<il::Typ> {
    type_result(expand_typ(&ctx.tdenv, typ)).and_then(|typ| match typ.node {
        il::TypKind::Iter(typ, il::Iter::List) => Attempt::ok(*typ),
        _ => Attempt::fail(destruct_error(TypeShape::List, typ.span)),
    })
}

fn as_struct_typ(ctx: &Context, typ: &il::Typ) -> Attempt<Vec<il::TypField>> {
    type_result(expand_typ(&ctx.tdenv, typ)).and_then(|typ| {
        let il::TypKind::Var(id, _) = &typ.node else {
            return Attempt::fail(destruct_error(TypeShape::Struct, typ.span));
        };
        let Some(TypeDef::Defined(_, def_typ)) = ctx.find_typdef_opt(id) else {
            return Attempt::fail(destruct_error(TypeShape::Struct, typ.span));
        };
        match &def_typ.node {
            il::DefTypKind::Struct(fields) => Attempt::ok(fields.clone()),
            _ => Attempt::fail(destruct_error(TypeShape::Struct, typ.span)),
        }
    })
}

fn arity_error(expected: usize, actual: usize, span: Span) -> ElabError {
    let mismatch = ArityMismatch::new(expected, actual);
    let mismatch = TypeArityMismatch::TypeArgument(mismatch);
    let type_error = TypeErrorKind::ArityMismatch(mismatch);
    ElabError::new(ElabErrorKind::ArityMismatch, span, type_error.to_string())
}

fn elab_plain_typ(ctx: &Context, plain_typ: &el::PlainTyp) -> Result<il::Typ, ElabError> {
    let kind = match &plain_typ.node {
        el::PlainTypKind::Bool => il::TypKind::Bool,
        el::PlainTypKind::Num(num_typ) => il::TypKind::Num(*num_typ),
        el::PlainTypKind::Text => il::TypKind::Text,
        el::PlainTypKind::Var(id, targs) => {
            let typdef = ctx.find_typdef(id)?;
            let tparams = typdef.tparams();
            if tparams.len() != targs.len() {
                return Err(arity_error(tparams.len(), targs.len(), id.span.clone()));
            }
            let mut targs_il = Vec::with_capacity(targs.len());
            for targ in targs {
                targs_il.push(elab_plain_typ(ctx, targ)?);
            }
            il::TypKind::Var(id.clone(), targs_il)
        }
        el::PlainTypKind::Paren(plain_typ) => elab_plain_typ(ctx, plain_typ)?.node,
        el::PlainTypKind::Tuple(plain_typs) => {
            let mut typs = Vec::with_capacity(plain_typs.len());
            for plain_typ in plain_typs {
                typs.push(elab_plain_typ(ctx, plain_typ)?);
            }
            il::TypKind::Tuple(typs)
        }
        el::PlainTypKind::Iter(plain_typ, iter) => {
            let typ = elab_plain_typ(ctx, plain_typ)?;
            il::TypKind::Iter(Box::new(typ), elab_iter(*iter))
        }
    };
    Ok(Spanned::new(kind, plain_typ.span.clone()))
}

fn elab_not_typ(ctx: &Context, typ: &el::Typ) -> Result<il::NotTyp, ElabError> {
    match typ {
        el::Typ::Plain(plain_typ) => {
            let typ = elab_plain_typ(ctx, plain_typ)?;
            Ok(Spanned::new(Mixfix::Arg(typ), plain_typ.span.clone()))
        }
        el::Typ::Notation(not_typ) => {
            let mixfix = match &not_typ.node {
                el::NotTypKind::Atom(atom) => Mixfix::Atom(atom.clone()),
                el::NotTypKind::Seq(typs) => {
                    let mut items = Vec::with_capacity(typs.len());
                    for typ in typs {
                        items.push(elab_not_typ(ctx, typ)?.node);
                    }
                    Mixfix::Seq(items)
                }
                el::NotTypKind::Infix(typ_l, atom, typ_r) => Mixfix::Infix(
                    Box::new(elab_not_typ(ctx, typ_l)?.node),
                    atom.clone(),
                    Box::new(elab_not_typ(ctx, typ_r)?.node),
                ),
                el::NotTypKind::Brack(atom_l, typ, atom_r) => Mixfix::Brack(
                    atom_l.clone(),
                    Box::new(elab_not_typ(ctx, typ)?.node),
                    atom_r.clone(),
                ),
            };
            Ok(Spanned::new(mixfix, not_typ.span.clone()))
        }
    }
}

fn elab_typ_case_plain(ctx: &Context, typ: &il::Typ) -> Result<Vec<il::TypCase>, ElabError> {
    let typ = expand_typ(&ctx.tdenv, typ)?;
    let il::TypKind::Var(id, targs) = &typ.node else {
        return Err(ElabError::new(
            ElabErrorKind::InvalidTypeExtension,
            typ.span,
            "cannot extend a non-variant type",
        ));
    };
    match ctx.find_typdef(id)? {
        TypeDef::Defining(_) => Err(ElabError::new(
            ElabErrorKind::InvalidTypeExtension,
            typ.span,
            "cannot extend an incomplete type",
        )),
        TypeDef::Defined(tparams, def_typ) => {
            let il::DefTypKind::Variant(cases) = &def_typ.node else {
                return Err(ElabError::new(
                    ElabErrorKind::InvalidTypeExtension,
                    typ.span,
                    "cannot extend a non-variant type",
                ));
            };
            let theta = Theta::from_lists(tparams, targs).map_err(|mismatch| {
                arity_error(mismatch.expected, mismatch.actual, typ.span.clone())
            })?;
            cases
                .iter()
                .map(|case| subst_typ_case(&theta, case).map_err(ElabError::from))
                .collect()
        }
        TypeDef::Parameter | TypeDef::Extern => Err(ElabError::new(
            ElabErrorKind::InvalidTypeExtension,
            typ.span,
            "cannot extend a non-variant type",
        )),
    }
}

fn elab_def_typ(
    ctx: &Context,
    id: &Id,
    tparams: &[el::TParam],
    def_typ: &el::DefTyp,
) -> Result<(TypeDef, il::DefTyp), ElabError> {
    let def_typ_il = match &def_typ.node {
        el::DefTypKind::Plain(plain_typ) => {
            let typ = elab_plain_typ(ctx, plain_typ)?;
            Spanned::new(il::DefTypKind::Plain(typ), plain_typ.span.clone())
        }
        el::DefTypKind::Struct(fields) => {
            let mut fields_il = Vec::with_capacity(fields.len());
            for (atom, plain_typ, _) in fields {
                fields_il.push((atom.clone(), elab_plain_typ(ctx, plain_typ)?));
            }
            Spanned::new(il::DefTypKind::Struct(fields_il), def_typ.span.clone())
        }
        el::DefTypKind::Variant(cases) => {
            let targs = tparams
                .iter()
                .map(|tparam| {
                    Spanned::new(
                        il::TypKind::Var(tparam.clone(), vec![]),
                        tparam.span.clone(),
                    )
                })
                .collect();
            let origin = Spanned::new((id.clone(), targs), id.span.clone());
            let mut cases_il = vec![];
            for (typ, hints) in cases {
                match typ {
                    el::Typ::Plain(plain_typ) => {
                        let typ = elab_plain_typ(ctx, plain_typ)?;
                        cases_il.extend(elab_typ_case_plain(ctx, &typ)?);
                    }
                    el::Typ::Notation(_) => {
                        let not_typ = elab_not_typ(ctx, typ)?;
                        cases_il.push((not_typ, origin.clone(), hints.clone()));
                    }
                }
            }
            for (index, case) in cases_il.iter().enumerate() {
                let mixop = case.0.node.to_mixop();
                if cases_il[..index]
                    .iter()
                    .any(|case_other| case_other.0.node.to_mixop() == mixop)
                {
                    return Err(ElabError::new(
                        ElabErrorKind::AmbiguousVariant,
                        def_typ.span.clone(),
                        "variant cases are ambiguous",
                    ));
                }
            }
            Spanned::new(il::DefTypKind::Variant(cases_il), def_typ.span.clone())
        }
    };
    let typdef = TypeDef::Defined(tparams.to_vec(), Box::new(def_typ_il.clone()));
    Ok((typdef, def_typ_il))
}

fn fail_attempt<T>(kind: ElabErrorKind, span: Span, message: impl Into<String>) -> Attempt<T> {
    Attempt::fail(ElabError::new(kind, span, message))
}

fn fail_infer<T>(span: Span, construct: &str) -> Attempt<T> {
    fail_attempt(
        ElabErrorKind::CannotInfer,
        span,
        format!("cannot infer type of {construct}"),
    )
}

fn inferred_exp(kind: il::ExpKind, typ: il::TypKind, span: Span) -> (il::Exp, il::Typ) {
    let exp = Spanned::new(Noted::new(kind, typ.clone()), span.clone());
    let typ = Spanned::new(typ, span);
    (exp, typ)
}

fn typ_at(kind: il::TypKind, span: &Span) -> il::Typ {
    Spanned::new(kind, span.clone())
}

fn operator_error<T>(span: Span) -> Attempt<T> {
    fail_attempt(
        ElabErrorKind::OperatorNotDefined,
        span,
        "operator is not defined for the operand types",
    )
}

fn infer_un_exp(
    ctx: Context,
    span: &Span,
    op: el::UnOp,
    exp: &el::Exp,
) -> Attempt<(Context, il::ExpKind, il::TypKind)> {
    let (ctx, exp, typ) = attempt!(infer_exp(ctx, exp));
    let candidates = match op {
        el::UnOp::Bool(_) => vec![(il::OpTyp::Bool, il::TypKind::Bool, il::TypKind::Bool)],
        el::UnOp::Num(_) => vec![
            (
                il::OpTyp::Nat,
                il::TypKind::Num(xl::num::Typ::Nat),
                il::TypKind::Num(xl::num::Typ::Nat),
            ),
            (
                il::OpTyp::Int,
                il::TypKind::Num(xl::num::Typ::Int),
                il::TypKind::Num(xl::num::Typ::Int),
            ),
        ],
    };
    for (op_typ, typ_operand, typ_result) in candidates {
        let expected = typ_at(typ_operand, &typ.span);
        if let Attempt::Ok(exp) = cast_exp(&ctx, &expected, &typ, exp.clone()) {
            return Attempt::ok((ctx, il::ExpKind::Un(op, op_typ, Box::new(exp)), typ_result));
        }
    }
    operator_error(span.clone())
}

fn infer_bin_exp(
    ctx: Context,
    span: &Span,
    exp_l: &el::Exp,
    op: el::BinOp,
    exp_r: &el::Exp,
) -> Attempt<(Context, il::ExpKind, il::TypKind)> {
    let (ctx, exp_l, typ_l) = attempt!(infer_exp(ctx, exp_l));
    let (ctx, exp_r, typ_r) = attempt!(infer_exp(ctx, exp_r));
    let candidates = match op {
        el::BinOp::Bool(_) => vec![(
            il::OpTyp::Bool,
            il::TypKind::Bool,
            il::TypKind::Bool,
            il::TypKind::Bool,
        )],
        el::BinOp::Num(xl::num::BinOp::Sub) => vec![
            (
                il::OpTyp::Int,
                il::TypKind::Num(xl::num::Typ::Nat),
                il::TypKind::Num(xl::num::Typ::Nat),
                il::TypKind::Num(xl::num::Typ::Int),
            ),
            (
                il::OpTyp::Int,
                il::TypKind::Num(xl::num::Typ::Int),
                il::TypKind::Num(xl::num::Typ::Int),
                il::TypKind::Num(xl::num::Typ::Int),
            ),
        ],
        el::BinOp::Num(_) => vec![
            (
                il::OpTyp::Nat,
                il::TypKind::Num(xl::num::Typ::Nat),
                il::TypKind::Num(xl::num::Typ::Nat),
                il::TypKind::Num(xl::num::Typ::Nat),
            ),
            (
                il::OpTyp::Int,
                il::TypKind::Num(xl::num::Typ::Int),
                il::TypKind::Num(xl::num::Typ::Int),
                il::TypKind::Num(xl::num::Typ::Int),
            ),
        ],
    };
    for (op_typ, expected_l, expected_r, result) in candidates {
        let expected_l = typ_at(expected_l, &typ_l.span);
        let expected_r = typ_at(expected_r, &typ_r.span);
        let Attempt::Ok(exp_l) = cast_exp(&ctx, &expected_l, &typ_l, exp_l.clone()) else {
            continue;
        };
        let Attempt::Ok(exp_r) = cast_exp(&ctx, &expected_r, &typ_r, exp_r.clone()) else {
            continue;
        };
        return Attempt::ok((
            ctx,
            il::ExpKind::Bin(op, op_typ, Box::new(exp_l), Box::new(exp_r)),
            result,
        ));
    }
    operator_error(span.clone())
}

fn infer_cmp_exp(
    ctx: Context,
    span: &Span,
    exp_l: &el::Exp,
    op: el::CmpOp,
    exp_r: &el::Exp,
) -> Attempt<(Context, il::ExpKind, il::TypKind)> {
    match op {
        el::CmpOp::Bool(_) => {
            let ctx_r = ctx.clone();
            let exp_l_r = exp_l.clone();
            let exp_r_r = exp_r.clone();
            let ctx_l = ctx;
            let exp_l_l = exp_l.clone();
            let exp_r_l = exp_r.clone();
            Attempt::choose_sequential(vec![
                Box::new(move || {
                    let (ctx, exp_r, typ_r) = attempt!(infer_exp(ctx_r, &exp_r_r));
                    let (ctx, exp_l) = attempt!(elab_exp(ctx, &typ_r, &exp_l_r));
                    Attempt::ok((
                        ctx,
                        il::ExpKind::Cmp(op, il::OpTyp::Bool, Box::new(exp_l), Box::new(exp_r)),
                        il::TypKind::Bool,
                    ))
                }),
                Box::new(move || {
                    let (ctx, exp_l, typ_l) = attempt!(infer_exp(ctx_l, &exp_l_l));
                    let (ctx, exp_r) = attempt!(elab_exp(ctx, &typ_l, &exp_r_l));
                    Attempt::ok((
                        ctx,
                        il::ExpKind::Cmp(op, il::OpTyp::Bool, Box::new(exp_l), Box::new(exp_r)),
                        il::TypKind::Bool,
                    ))
                }),
            ])
        }
        el::CmpOp::Num(_) => {
            let (ctx, exp_l, typ_l) = attempt!(infer_exp(ctx, exp_l));
            let (ctx, exp_r, typ_r) = attempt!(infer_exp(ctx, exp_r));
            for (op_typ, expected_kind) in [
                (il::OpTyp::Nat, il::TypKind::Num(xl::num::Typ::Nat)),
                (il::OpTyp::Int, il::TypKind::Num(xl::num::Typ::Int)),
            ] {
                let expected_l = typ_at(expected_kind.clone(), &typ_l.span);
                let expected_r = typ_at(expected_kind, &typ_r.span);
                let Attempt::Ok(exp_l) = cast_exp(&ctx, &expected_l, &typ_l, exp_l.clone()) else {
                    continue;
                };
                let Attempt::Ok(exp_r) = cast_exp(&ctx, &expected_r, &typ_r, exp_r.clone()) else {
                    continue;
                };
                return Attempt::ok((
                    ctx,
                    il::ExpKind::Cmp(op, op_typ, Box::new(exp_l), Box::new(exp_r)),
                    il::TypKind::Bool,
                ));
            }
            operator_error(span.clone())
        }
    }
}

fn infer_exps(
    mut ctx: Context,
    exps: &[el::Exp],
) -> Attempt<(Context, Vec<il::Exp>, Vec<il::Typ>)> {
    let mut exps_il = Vec::with_capacity(exps.len());
    let mut typs_il = Vec::with_capacity(exps.len());
    for exp in exps {
        let (ctx_next, exp_il, typ_il) = attempt!(infer_exp(ctx, exp));
        ctx = ctx_next;
        exps_il.push(exp_il);
        typs_il.push(typ_il);
    }
    Attempt::ok((ctx, exps_il, typs_il))
}

fn infer_exp(ctx: Context, exp: &el::Exp) -> Attempt<(Context, il::Exp, il::Typ)> {
    let span = exp.span.clone();
    let (ctx, kind, typ) = match &exp.node {
        el::ExpKind::Bool(value) => (ctx, il::ExpKind::Bool(*value), il::TypKind::Bool),
        el::ExpKind::Num(_, value) => (
            ctx,
            il::ExpKind::Num(value.clone()),
            il::TypKind::Num(xl::num::to_typ(value)),
        ),
        el::ExpKind::Text(value) => (ctx, il::ExpKind::Text(value.clone()), il::TypKind::Text),
        el::ExpKind::Var(id) => {
            let tid = xl::var::strip_var_suffix(id);
            let Some(typ) = ctx.find_metavar_opt(&tid) else {
                return fail_infer(id.span.clone(), "variable");
            };
            (ctx.clone(), il::ExpKind::Var(id.clone()), typ.node.clone())
        }
        el::ExpKind::Un(op, exp_inner) => {
            let (ctx, kind, typ) = attempt!(infer_un_exp(ctx, &span, *op, exp_inner));
            (ctx, kind, typ)
        }
        el::ExpKind::Bin(exp_l, op, exp_r) => {
            let (ctx, kind, typ) = attempt!(infer_bin_exp(ctx, &span, exp_l, *op, exp_r));
            (ctx, kind, typ)
        }
        el::ExpKind::Cmp(exp_l, op, exp_r) => {
            let (ctx, kind, typ) = attempt!(infer_cmp_exp(ctx, &span, exp_l, *op, exp_r));
            (ctx, kind, typ)
        }
        el::ExpKind::Arith(exp_inner) | el::ExpKind::Paren(exp_inner) => {
            let (ctx, exp_il, typ) = attempt!(infer_exp(ctx, exp_inner));
            (ctx, exp_il.node.kind, typ.node)
        }
        el::ExpKind::List(exps) => {
            let Some((exp_first, exps_rest)) = exps.split_first() else {
                return fail_infer(span, "empty list");
            };
            let (ctx, exp_first, typ_first) = attempt!(infer_exp(ctx, exp_first));
            let (ctx, mut exps_rest, typs_rest) = attempt!(infer_exps(ctx, exps_rest));
            for typ in &typs_rest {
                let equivalent = attempt!(type_result(equiv_typ(&ctx.tdenv, &typ_first, typ)));
                if !equivalent {
                    return fail_infer(span, "list with heterogeneous elements");
                }
            }
            let mut exps_il = vec![exp_first];
            exps_il.append(&mut exps_rest);
            let typ = il::TypKind::Iter(Box::new(typ_first), il::Iter::List);
            (ctx, il::ExpKind::List(exps_il), typ)
        }
        el::ExpKind::Cons(exp_head, exp_tail) => {
            let (ctx, exp_head, typ_head) = attempt!(infer_exp(ctx, exp_head));
            let typ_list = Spanned::new(
                il::TypKind::Iter(Box::new(typ_head.clone()), il::Iter::List),
                typ_head.span.clone(),
            );
            let (ctx, exp_tail) = attempt!(elab_exp(ctx, &typ_list, exp_tail));
            (
                ctx,
                il::ExpKind::Cons(Box::new(exp_head), Box::new(exp_tail)),
                typ_list.node,
            )
        }
        el::ExpKind::Cat(exp_l, exp_r) => {
            let ctx_list = ctx.clone();
            let exp_l_list = exp_l.clone();
            let exp_r_list = exp_r.clone();
            let ctx_text = ctx;
            let exp_l_text = exp_l.clone();
            let exp_r_text = exp_r.clone();
            let (ctx, kind, typ) = attempt!(Attempt::choose_sequential(vec![
                Box::new(move || {
                    let (ctx, exp_l, typ_l) = attempt!(infer_exp(ctx_list, &exp_l_list));
                    let typ_base = attempt!(as_list_typ(&ctx, &typ_l));
                    let typ_list = Spanned::new(
                        il::TypKind::Iter(Box::new(typ_base.clone()), il::Iter::List),
                        typ_base.span,
                    );
                    let (ctx, exp_r) = attempt!(elab_exp(ctx, &typ_list, &exp_r_list));
                    Attempt::ok((
                        ctx,
                        il::ExpKind::Cat(Box::new(exp_l), Box::new(exp_r)),
                        typ_list.node,
                    ))
                }),
                Box::new(move || {
                    let typ_text = typ_at(il::TypKind::Text, &exp_l_text.span);
                    let (ctx, exp_l) = attempt!(elab_exp(ctx_text, &typ_text, &exp_l_text));
                    let typ_text_r = typ_at(il::TypKind::Text, &exp_r_text.span);
                    let (ctx, exp_r) = attempt!(elab_exp(ctx, &typ_text_r, &exp_r_text));
                    Attempt::ok((
                        ctx,
                        il::ExpKind::Cat(Box::new(exp_l), Box::new(exp_r)),
                        il::TypKind::Text,
                    ))
                }),
            ]));
            (ctx, kind, typ)
        }
        el::ExpKind::Idx(exp_base, exp_index) => {
            let ctx_list = ctx.clone();
            let exp_base_list = exp_base.clone();
            let exp_index_list = exp_index.clone();
            let ctx_text = ctx;
            let exp_base_text = exp_base.clone();
            let exp_index_text = exp_index.clone();
            let (ctx, kind, typ) = attempt!(Attempt::choose_sequential(vec![
                Box::new(move || {
                    let (ctx, exp_base, typ_base) = attempt!(infer_exp(ctx_list, &exp_base_list));
                    let typ_element = attempt!(as_list_typ(&ctx, &typ_base));
                    let typ_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_index_list.span);
                    let (ctx, exp_index) = attempt!(elab_exp(ctx, &typ_nat, &exp_index_list));
                    Attempt::ok((
                        ctx,
                        il::ExpKind::Idx(Box::new(exp_base), Box::new(exp_index)),
                        typ_element.node,
                    ))
                }),
                Box::new(move || {
                    let typ_text = typ_at(il::TypKind::Text, &exp_base_text.span);
                    let (ctx, exp_base) = attempt!(elab_exp(ctx_text, &typ_text, &exp_base_text));
                    let typ_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_index_text.span);
                    let (ctx, exp_index) = attempt!(elab_exp(ctx, &typ_nat, &exp_index_text));
                    Attempt::ok((
                        ctx,
                        il::ExpKind::Idx(Box::new(exp_base), Box::new(exp_index)),
                        il::TypKind::Text,
                    ))
                }),
            ]));
            (ctx, kind, typ)
        }
        el::ExpKind::Slice(exp_base, exp_index, exp_length) => {
            let ctx_list = ctx.clone();
            let exp_base_list = exp_base.clone();
            let exp_index_list = exp_index.clone();
            let exp_length_list = exp_length.clone();
            let ctx_text = ctx;
            let exp_base_text = exp_base.clone();
            let exp_index_text = exp_index.clone();
            let exp_length_text = exp_length.clone();
            let (ctx, kind, typ) = attempt!(Attempt::choose_sequential(vec![
                Box::new(move || {
                    let (ctx, exp_base, typ_base) = attempt!(infer_exp(ctx_list, &exp_base_list));
                    attempt!(as_list_typ(&ctx, &typ_base));
                    let typ_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_index_list.span);
                    let (ctx, exp_index) = attempt!(elab_exp(ctx, &typ_nat, &exp_index_list));
                    let typ_nat =
                        typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_length_list.span);
                    let (ctx, exp_length) = attempt!(elab_exp(ctx, &typ_nat, &exp_length_list));
                    Attempt::ok((
                        ctx,
                        il::ExpKind::Slice(
                            Box::new(exp_base),
                            Box::new(exp_index),
                            Box::new(exp_length),
                        ),
                        typ_base.node,
                    ))
                }),
                Box::new(move || {
                    let typ_text = typ_at(il::TypKind::Text, &exp_base_text.span);
                    let (ctx, exp_base) = attempt!(elab_exp(ctx_text, &typ_text, &exp_base_text));
                    let typ_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_index_text.span);
                    let (ctx, exp_index) = attempt!(elab_exp(ctx, &typ_nat, &exp_index_text));
                    let typ_nat =
                        typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_length_text.span);
                    let (ctx, exp_length) = attempt!(elab_exp(ctx, &typ_nat, &exp_length_text));
                    Attempt::ok((
                        ctx,
                        il::ExpKind::Slice(
                            Box::new(exp_base),
                            Box::new(exp_index),
                            Box::new(exp_length),
                        ),
                        il::TypKind::Text,
                    ))
                }),
            ]));
            (ctx, kind, typ)
        }
        el::ExpKind::Tuple(exps) => {
            let (ctx, exps, typs) = attempt!(infer_exps(ctx, exps));
            (ctx, il::ExpKind::Tuple(exps), il::TypKind::Tuple(typs))
        }
        el::ExpKind::Len(exp_inner) => {
            let ctx_list = ctx.clone();
            let exp_list = exp_inner.clone();
            let ctx_text = ctx;
            let exp_text = exp_inner.clone();
            let (ctx, kind, typ) = attempt!(Attempt::choose_sequential(vec![
                Box::new(move || {
                    let (ctx, exp, typ) = attempt!(infer_exp(ctx_list, &exp_list));
                    attempt!(as_list_typ(&ctx, &typ));
                    Attempt::ok((
                        ctx,
                        il::ExpKind::Len(Box::new(exp)),
                        il::TypKind::Num(xl::num::Typ::Nat),
                    ))
                }),
                Box::new(move || {
                    let typ_text = typ_at(il::TypKind::Text, &exp_text.span);
                    let (ctx, exp) = attempt!(elab_exp(ctx_text, &typ_text, &exp_text));
                    Attempt::ok((
                        ctx,
                        il::ExpKind::Len(Box::new(exp)),
                        il::TypKind::Num(xl::num::Typ::Nat),
                    ))
                }),
            ]));
            (ctx, kind, typ)
        }
        el::ExpKind::Mem(exp_element, exp_set) => {
            let ctx_element = ctx.clone();
            let exp_element_l = exp_element.clone();
            let exp_set_l = exp_set.clone();
            let ctx_set = ctx;
            let exp_element_r = exp_element.clone();
            let exp_set_r = exp_set.clone();
            let (ctx, kind, typ) = attempt!(Attempt::choose_sequential(vec![
                Box::new(move || {
                    let (ctx, exp_element, typ_element) =
                        attempt!(infer_exp(ctx_element, &exp_element_l));
                    let typ_list = Spanned::new(
                        il::TypKind::Iter(Box::new(typ_element), il::Iter::List),
                        exp_set_l.span.clone(),
                    );
                    let (ctx, exp_set) = attempt!(elab_exp(ctx, &typ_list, &exp_set_l));
                    Attempt::ok((
                        ctx,
                        il::ExpKind::Mem(Box::new(exp_element), Box::new(exp_set)),
                        il::TypKind::Bool,
                    ))
                }),
                Box::new(move || {
                    let (ctx, exp_set, typ_set) = attempt!(infer_exp(ctx_set, &exp_set_r));
                    let typ_element = attempt!(as_list_typ(&ctx, &typ_set));
                    let (ctx, exp_element) = attempt!(elab_exp(ctx, &typ_element, &exp_element_r));
                    Attempt::ok((
                        ctx,
                        il::ExpKind::Mem(Box::new(exp_element), Box::new(exp_set)),
                        il::TypKind::Bool,
                    ))
                }),
            ]));
            (ctx, kind, typ)
        }
        el::ExpKind::Dot(exp_inner, atom) => {
            let (ctx, exp_inner, typ_inner) = attempt!(infer_exp(ctx, exp_inner));
            let fields = attempt!(as_struct_typ(&ctx, &typ_inner));
            let Some((_, typ_field)) = fields
                .iter()
                .find(|(atom_field, _)| atom_field.node == atom.node)
            else {
                return fail_infer(atom.span.clone(), "field");
            };
            (
                ctx,
                il::ExpKind::Dot(Box::new(exp_inner), atom.clone()),
                typ_field.node.clone(),
            )
        }
        el::ExpKind::Upd(exp_base, path, exp_field) => {
            let (ctx, exp_base, typ_base) = attempt!(infer_exp(ctx, exp_base));
            let (ctx, path, typ_field) = attempt!(elab_path(ctx, &typ_base, path));
            let (ctx, exp_field) = attempt!(elab_exp(ctx, &typ_field, exp_field));
            (
                ctx,
                il::ExpKind::Upd(Box::new(exp_base), path, Box::new(exp_field)),
                typ_base.node,
            )
        }
        el::ExpKind::Call(id, targs, args) => {
            let (tparams, params, typ_ret) = match ctx.find_func_signature(id) {
                Ok((tparams, params, typ_ret)) => {
                    (tparams.to_vec(), params.to_vec(), typ_ret.clone())
                }
                Err(error) => return Attempt::fail(error),
            };
            if tparams.len() != targs.len() {
                return Attempt::fail(arity_error(tparams.len(), targs.len(), id.span.clone()));
            }
            let mut targs_il = Vec::with_capacity(targs.len());
            for targ in targs {
                let targ_il = match elab_plain_typ(&ctx, targ) {
                    Ok(targ_il) => targ_il,
                    Err(error) => return Attempt::fail(error),
                };
                targs_il.push(targ_il);
            }
            let theta = match Theta::from_lists(&tparams, &targs_il) {
                Ok(theta) => theta,
                Err(mismatch) => {
                    return Attempt::fail(arity_error(
                        mismatch.expected,
                        mismatch.actual,
                        id.span.clone(),
                    ));
                }
            };
            let params = attempt!(type_result(subst_params(&theta, &params)));
            let typ_ret = attempt!(type_result(subst_typ(&theta, &typ_ret)));
            let (ctx, args) = attempt!(elab_args(ctx, &params, args, false, &span));
            (
                ctx,
                il::ExpKind::Call(id.clone(), targs_il, args),
                typ_ret.node,
            )
        }
        el::ExpKind::Sub(exp_inner, plain_typ) => {
            let (ctx, exp_inner, typ_source) = attempt!(infer_exp(ctx, exp_inner));
            let typ_target = match elab_plain_typ(&ctx, plain_typ) {
                Ok(typ) => typ,
                Err(error) => return Attempt::fail(error),
            };
            let source_sub = attempt!(type_result(sub_typ(&ctx.tdenv, &typ_source, &typ_target,)));
            let target_sub = attempt!(type_result(sub_typ(&ctx.tdenv, &typ_target, &typ_source,)));
            if !source_sub && !target_sub {
                return fail_attempt(
                    ElabErrorKind::TypeMismatch,
                    exp_inner.span.clone(),
                    "subtype expression compares incomparable types",
                );
            }
            let check = attempt!(type_result(optimize_sub_typ(
                &ctx.tdenv,
                &typ_source,
                &typ_target,
            )));
            (
                ctx,
                il::ExpKind::Sub(Box::new(exp_inner), typ_target, Box::new(check)),
                il::TypKind::Bool,
            )
        }
        el::ExpKind::Iter(exp_inner, iter) => {
            let (ctx, exp_inner, typ_inner) = attempt!(infer_exp(ctx, exp_inner));
            let iter = elab_iter(*iter);
            (
                ctx,
                il::ExpKind::Iter(Box::new(exp_inner), (iter, vec![])),
                il::TypKind::Iter(Box::new(typ_inner), iter),
            )
        }
        el::ExpKind::Eps => return fail_infer(span, "empty sequence"),
        el::ExpKind::Str(_) => return fail_infer(span, "struct expression"),
        el::ExpKind::Atom(_) => return fail_infer(span, "atom"),
        el::ExpKind::Seq(_) => return fail_infer(span, "sequence expression"),
        el::ExpKind::Infix(_, _, _) => return fail_infer(span, "infix expression"),
        el::ExpKind::Brack(_, _, _) => return fail_infer(span, "bracket expression"),
        el::ExpKind::Hole(_)
        | el::ExpKind::Fuse(_, _)
        | el::ExpKind::Unparen(_)
        | el::ExpKind::Latex(_) => {
            return fail_attempt(
                ElabErrorKind::MisplacedConstruct,
                span,
                "construct is misplaced during elaboration",
            );
        }
    };
    let (exp, typ) = inferred_exp(kind, typ, exp.span.clone());
    Attempt::ok((ctx, exp, typ))
}

fn cast_exp(
    ctx: &Context,
    typ_expect: &il::Typ,
    typ_infer: &il::Typ,
    exp: il::Exp,
) -> Attempt<il::Exp> {
    let equivalent = attempt!(type_result(equiv_typ(&ctx.tdenv, typ_expect, typ_infer)));
    if equivalent {
        return Attempt::ok(exp);
    }
    let subtype = attempt!(type_result(sub_typ(&ctx.tdenv, typ_infer, typ_expect)));
    if subtype {
        let node = Noted::new(
            il::ExpKind::UpCast(typ_expect.clone(), Box::new(exp.clone())),
            typ_expect.node.clone(),
        );
        return Attempt::ok(Spanned::new(node, exp.span));
    }
    fail_attempt(
        ElabErrorKind::InvalidCast,
        exp.span,
        "cannot cast inferred expression to expected type",
    )
}

fn respan_parenthesized_exp(exp: &mut il::Exp, span: &Span) {
    exp.span = span.clone();
    match &mut exp.node.kind {
        il::ExpKind::UpCast(_, exp_inner) | il::ExpKind::DownCast(_, exp_inner) => {
            respan_parenthesized_exp(exp_inner, span);
        }
        _ => {}
    }
}

fn elab_exp(ctx: Context, typ_expect: &il::Typ, exp: &el::Exp) -> Attempt<(Context, il::Exp)> {
    let error = ElabError::new(
        ElabErrorKind::NoMatchingAlternative,
        exp.span.clone(),
        "expression elaboration failed",
    );
    let parenthesized = matches!(exp.node, el::ExpKind::Paren(_));
    let span = exp.span.clone();
    elab_exp_inner(ctx, typ_expect, exp)
        .map(move |(ctx, mut exp)| {
            if parenthesized {
                respan_parenthesized_exp(&mut exp, &span);
            }
            (ctx, exp)
        })
        .nest(error)
}

fn elab_exp_inner(
    ctx: Context,
    typ_expect: &il::Typ,
    exp: &el::Exp,
) -> Attempt<(Context, il::Exp)> {
    if let Attempt::Ok((typ_base, iter)) = as_iter_typ(&ctx, typ_expect) {
        let ctx_wrap = ctx.clone();
        let typ_expect_wrap = typ_expect.clone();
        let exp_wrap = exp.clone();
        let ctx_normal = ctx.clone();
        let typ_normal = typ_expect.clone();
        let exp_normal = exp.clone();
        return Attempt::choose_sequential(vec![
            Box::new(move || {
                if matches!(&exp_wrap.node, el::ExpKind::Var(id) if id.node == "_")
                    || matches!(&exp_wrap.node, el::ExpKind::Eps)
                    || matches!(&exp_wrap.node, el::ExpKind::List(exps) if exps.is_empty())
                {
                    return Attempt::fail_silent();
                }
                let (ctx, exp) = attempt!(elab_exp(ctx_wrap, &typ_base, &exp_wrap));
                let kind = match iter {
                    il::Iter::Opt => il::ExpKind::Opt(Some(Box::new(exp))),
                    il::Iter::List => il::ExpKind::List(vec![exp]),
                };
                let exp = Spanned::new(
                    Noted::new(kind, typ_expect_wrap.node.clone()),
                    exp_wrap.span,
                );
                Attempt::ok((ctx, exp))
            }),
            Box::new(move || elab_exp_normal(ctx_normal, &typ_normal, &exp_normal)),
        ]);
    }
    elab_exp_normal(ctx, typ_expect, exp)
}

fn elab_exp_normal(
    ctx: Context,
    typ_expect: &il::Typ,
    exp: &el::Exp,
) -> Attempt<(Context, il::Exp)> {
    match infer_exp(ctx.clone(), exp) {
        Attempt::Ok((ctx, exp, typ_infer)) => {
            let exp = attempt!(cast_exp(&ctx, typ_expect, &typ_infer, exp));
            Attempt::ok((ctx, exp))
        }
        Attempt::Fail(_) => elab_exp_contextual(ctx, typ_expect, exp),
    }
}

fn elab_path(
    ctx: Context,
    typ_expect: &il::Typ,
    path: &el::Path,
) -> Attempt<(Context, il::Path, il::Typ)> {
    match &path.node {
        el::PathKind::Root => {
            let path_il = Spanned::new(
                Noted::new(il::PathKind::Root, typ_expect.node.clone()),
                path.span.clone(),
            );
            let typ = Spanned::new(typ_expect.node.clone(), path.span.clone());
            Attempt::ok((ctx, path_il, typ))
        }
        el::PathKind::Idx(path_inner, exp_index) => {
            let ctx_list = ctx.clone();
            let typ_list = typ_expect.clone();
            let path_list = path_inner.clone();
            let index_list = exp_index.clone();
            let ctx_text = ctx;
            let typ_text = typ_expect.clone();
            let path_text = path_inner.clone();
            let index_text = exp_index.clone();
            Attempt::choose_sequential(vec![
                Box::new(move || {
                    let (ctx, path_inner, typ_inner) =
                        attempt!(elab_path(ctx_list, &typ_list, &path_list));
                    let typ_element = attempt!(as_list_typ(&ctx, &typ_inner));
                    let typ_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &index_list.span);
                    let (ctx, index) = attempt!(elab_exp(ctx, &typ_nat, &index_list));
                    let path_il = Spanned::new(
                        Noted::new(
                            il::PathKind::Idx(Box::new(path_inner), Box::new(index)),
                            typ_element.node.clone(),
                        ),
                        path.span.clone(),
                    );
                    let typ_element = Spanned::new(typ_element.node, path.span.clone());
                    Attempt::ok((ctx, path_il, typ_element))
                }),
                Box::new(move || {
                    let (ctx, path_inner, typ_inner) =
                        attempt!(elab_path(ctx_text, &typ_text, &path_text));
                    attempt!(as_text_typ(&ctx, &typ_inner));
                    let typ_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &index_text.span);
                    let (ctx, index) = attempt!(elab_exp(ctx, &typ_nat, &index_text));
                    let path_il = Spanned::new(
                        Noted::new(
                            il::PathKind::Idx(Box::new(path_inner), Box::new(index)),
                            typ_inner.node.clone(),
                        ),
                        path.span.clone(),
                    );
                    let typ_inner = Spanned::new(typ_inner.node, path.span.clone());
                    Attempt::ok((ctx, path_il, typ_inner))
                }),
            ])
        }
        el::PathKind::Slice(path_inner, exp_index, exp_length) => {
            let (ctx, path_inner, typ_inner) = attempt!(elab_path(ctx, typ_expect, path_inner));
            let is_list = matches!(as_list_typ(&ctx, &typ_inner), Attempt::Ok(_));
            let is_text = matches!(as_text_typ(&ctx, &typ_inner), Attempt::Ok(_));
            if !is_list && !is_text {
                return fail_attempt(
                    ElabErrorKind::CannotDestructure(TypeShape::List),
                    typ_inner.span,
                    "slice path requires a list or text",
                );
            }
            let typ_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_index.span);
            let (ctx, exp_index) = attempt!(elab_exp(ctx, &typ_nat, exp_index));
            let typ_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_length.span);
            let (ctx, exp_length) = attempt!(elab_exp(ctx, &typ_nat, exp_length));
            let path_il = Spanned::new(
                Noted::new(
                    il::PathKind::Slice(
                        Box::new(path_inner),
                        Box::new(exp_index),
                        Box::new(exp_length),
                    ),
                    typ_inner.node.clone(),
                ),
                path.span.clone(),
            );
            let typ_inner = Spanned::new(typ_inner.node, path.span.clone());
            Attempt::ok((ctx, path_il, typ_inner))
        }
        el::PathKind::Dot(path_inner, atom) => {
            let (ctx, path_inner, typ_inner) = attempt!(elab_path(ctx, typ_expect, path_inner));
            let fields = attempt!(as_struct_typ(&ctx, &typ_inner));
            let Some((_, typ_field)) = fields
                .into_iter()
                .find(|(atom_field, _)| atom_field.node == atom.node)
            else {
                return fail_infer(atom.span.clone(), "field");
            };
            let path_il = Spanned::new(
                Noted::new(
                    il::PathKind::Dot(Box::new(path_inner), atom.clone()),
                    typ_field.node.clone(),
                ),
                path.span.clone(),
            );
            let typ_field = Spanned::new(typ_field.node, path.span.clone());
            Attempt::ok((ctx, path_il, typ_field))
        }
    }
}

fn elab_param(ctx: &Context, param: &el::Param) -> Result<il::Param, ElabError> {
    let kind = match &param.node {
        el::ParamKind::Exp(typ) => il::ParamKind::Exp(elab_plain_typ(ctx, typ)?),
        el::ParamKind::Def(id, tparams, params, typ_ret) => {
            let mut seen = std::collections::HashSet::new();
            if !tparams
                .iter()
                .all(|tparam| seen.insert(tparam.node.clone()))
            {
                return Err(ElabError::new(
                    ElabErrorKind::Duplicate(EntityKind::Type),
                    id.span.clone(),
                    "type parameters are not distinct",
                ));
            }
            let ctx_local = ctx.clone().add_tparams(tparams)?;
            let params = params
                .iter()
                .map(|param| elab_param(&ctx_local, param))
                .collect::<Result<Vec<_>, _>>()?;
            let typ_ret = elab_plain_typ(&ctx_local, typ_ret)?;
            il::ParamKind::Def(id.clone(), tparams.clone(), params, typ_ret)
        }
    };
    Ok(Spanned::new(kind, param.span.clone()))
}

fn typ_of_param(param: &il::Param) -> il::Typ {
    match &param.node {
        il::ParamKind::Exp(typ) => typ.clone(),
        il::ParamKind::Def(_, tparams, params, typ_ret) => Spanned::new(
            il::TypKind::Func(il::FuncTyp {
                tparams: tparams.clone(),
                typs_params: params.iter().map(typ_of_param).collect(),
                typ_ret: Box::new(typ_ret.clone()),
            }),
            param.span.clone(),
        ),
    }
}

fn elab_arg(
    ctx: Context,
    param: &il::Param,
    arg: &el::Arg,
    as_def: bool,
) -> Attempt<(Context, il::Arg)> {
    match (&param.node, &arg.node) {
        (il::ParamKind::Exp(typ), el::ArgKind::Exp(exp)) => {
            let (ctx, exp) = attempt!(elab_exp(ctx, typ, exp));
            Attempt::ok((
                ctx,
                Spanned::new(il::ArgKind::Exp(Box::new(exp)), arg.span.clone()),
            ))
        }
        (il::ParamKind::Def(id_param, tparams, params, typ_ret), el::ArgKind::Def(id_arg))
            if as_def =>
        {
            if id_param.node != id_arg.node {
                return fail_attempt(
                    ElabErrorKind::InvalidArgument,
                    arg.span.clone(),
                    "function argument does not match its declared parameter",
                );
            }
            let ctx = match ctx.add_defined_func(
                id_param.clone(),
                tparams.clone(),
                params.clone(),
                typ_ret.clone(),
            ) {
                Ok(ctx) => ctx,
                Err(error) => return Attempt::fail(error),
            };
            Attempt::ok((
                ctx,
                Spanned::new(il::ArgKind::Def(id_arg.clone()), arg.span.clone()),
            ))
        }
        (il::ParamKind::Def(_, tparams, params, typ_ret), el::ArgKind::Def(id_arg)) => {
            let (tparams_arg, params_arg, typ_ret_arg) = match ctx.find_func_signature(id_arg) {
                Ok(signature) => signature,
                Err(error) => return Attempt::fail(error),
            };
            let typ_param = il::FuncTyp {
                tparams: tparams.clone(),
                typs_params: params.iter().map(typ_of_param).collect(),
                typ_ret: Box::new(typ_ret.clone()),
            };
            let typ_arg = il::FuncTyp {
                tparams: tparams_arg.to_vec(),
                typs_params: params_arg.iter().map(typ_of_param).collect(),
                typ_ret: Box::new(typ_ret_arg.clone()),
            };
            let equivalent = attempt!(type_result(equiv_func_typ(
                &ctx.tdenv, &arg.span, &typ_param, &typ_arg,
            )));
            if !equivalent {
                return fail_attempt(
                    ElabErrorKind::InvalidArgument,
                    arg.span.clone(),
                    "function argument type does not match",
                );
            }
            Attempt::ok((
                ctx,
                Spanned::new(il::ArgKind::Def(id_arg.clone()), arg.span.clone()),
            ))
        }
        _ => fail_attempt(
            ElabErrorKind::InvalidArgument,
            arg.span.clone(),
            "argument kind does not match parameter kind",
        ),
    }
}

fn elab_args(
    mut ctx: Context,
    params: &[il::Param],
    args: &[el::Arg],
    as_def: bool,
    span: &Span,
) -> Attempt<(Context, Vec<il::Arg>)> {
    if params.len() != args.len() {
        return fail_attempt(
            ElabErrorKind::ArityMismatch,
            span.clone(),
            "argument count does not match parameter count",
        );
    }
    let mut args_il = Vec::with_capacity(args.len());
    for (param, arg) in params.iter().zip(args) {
        let (ctx_next, arg) = attempt!(elab_arg(ctx, param, arg, as_def));
        ctx = ctx_next;
        args_il.push(arg);
    }
    Attempt::ok((ctx, args_il))
}

#[derive(Clone, Debug)]
enum PremInternal {
    Some(il::Prem),
    Var(Span),
    Else(Span),
}

fn elab_relation_prem(
    ctx: Context,
    prem_span: &Span,
    id: &Id,
    exp: &el::Exp,
    negated: bool,
) -> Attempt<(Context, PremInternal)> {
    let (not_typ, input_hint) = match ctx.find_rel_signature(id) {
        Ok((not_typ, input_hint)) => (not_typ.clone(), input_hint.clone()),
        Err(error) => return Attempt::fail(error),
    };
    let (ctx, not_exp) = attempt!(elab_not_exp(ctx, &not_typ, exp));
    let args = not_exp.args();
    let conditional = match input::is_conditional(&input_hint, &args) {
        Ok(conditional) => conditional,
        Err(error) => {
            return fail_attempt(
                ElabErrorKind::InvalidInputHint,
                exp.span.clone(),
                error.to_string(),
            );
        }
    };
    let kind = if negated {
        if !conditional {
            return fail_attempt(
                ElabErrorKind::InvalidPremise,
                exp.span.clone(),
                "negated rule premise takes outputs",
            );
        }
        il::PremKind::IfNotHold(il::IfNotHoldPrem {
            id: id.clone(),
            not_exp,
        })
    } else if conditional {
        il::PremKind::IfHold(il::IfHoldPrem {
            id: id.clone(),
            not_exp,
        })
    } else {
        il::PremKind::Rule(il::RulePrem {
            id: id.clone(),
            not_exp,
            input_hint,
        })
    };
    let prem = Spanned::new(kind, prem_span.clone());
    Attempt::ok((ctx, PremInternal::Some(prem)))
}

fn elab_prem(ctx: Context, prem: &el::Prem) -> Attempt<(Context, PremInternal)> {
    match &prem.node {
        el::PremKind::Var(var_prem) => {
            if xl::var::strip_var_suffix(&var_prem.id).node != var_prem.id.node {
                return fail_attempt(
                    ElabErrorKind::InvalidIdentifier,
                    var_prem.id.span.clone(),
                    "invalid meta-variable identifier",
                );
            }
            if ctx.bound_typdef(&var_prem.id) {
                return fail_attempt(
                    ElabErrorKind::Duplicate(EntityKind::Type),
                    var_prem.id.span.clone(),
                    "type already defined",
                );
            }
            let typ = match elab_plain_typ(&ctx, &var_prem.plain_typ) {
                Ok(typ) => typ,
                Err(error) => return Attempt::fail(error),
            };
            let ctx = match ctx.add_metavar(var_prem.id.clone(), typ) {
                Ok(ctx) => ctx,
                Err(error) => return Attempt::fail(error),
            };
            Attempt::ok((ctx, PremInternal::Var(prem.span.clone())))
        }
        el::PremKind::Rule(rule_prem) => {
            elab_relation_prem(ctx, &prem.span, &rule_prem.id, &rule_prem.exp, false)
        }
        el::PremKind::RuleNot(rule_prem) => {
            elab_relation_prem(ctx, &prem.span, &rule_prem.id, &rule_prem.exp, true)
        }
        el::PremKind::If(if_prem) => {
            let typ_bool = typ_at(il::TypKind::Bool, &if_prem.exp.span);
            let (ctx, exp) = attempt!(elab_exp(ctx, &typ_bool, &if_prem.exp));
            let prem = Spanned::new(il::PremKind::If(il::IfPrem { exp }), prem.span.clone());
            Attempt::ok((ctx, PremInternal::Some(prem)))
        }
        el::PremKind::Else => Attempt::ok((ctx, PremInternal::Else(prem.span.clone()))),
        el::PremKind::Iter(iter_prem) => {
            let (ctx, prem_inner) = attempt!(elab_prem(ctx, &iter_prem.prem));
            let PremInternal::Some(prem_inner) = prem_inner else {
                return fail_attempt(
                    ElabErrorKind::InvalidIteration,
                    iter_prem.prem.span.clone(),
                    "cannot iterate variable or otherwise premise",
                );
            };
            let iter_prem = il::IterPrem {
                iter: elab_iter(iter_prem.iter),
                vars_bound: vec![],
                vars_bind: vec![],
            };
            let prem = Spanned::new(
                il::PremKind::Iter(il::IteratedPrem {
                    prem: Box::new(prem_inner),
                    iter_prem,
                }),
                prem.span.clone(),
            );
            Attempt::ok((ctx, PremInternal::Some(prem)))
        }
        el::PremKind::Debug(debug_prem) => {
            let (ctx, exp, _) = attempt!(infer_exp(ctx, &debug_prem.exp));
            let prem = Spanned::new(
                il::PremKind::Debug(il::DebugPrem { exp }),
                prem.span.clone(),
            );
            Attempt::ok((ctx, PremInternal::Some(prem)))
        }
    }
}

fn elab_prems(
    mut ctx: Context,
    prems: &[el::Prem],
    span: &Span,
) -> Attempt<(Context, Vec<il::Prem>, bool)> {
    let mut prems_il = Vec::new();
    let mut else_count = 0;
    for prem in prems {
        let (ctx_next, prem) = attempt!(elab_prem(ctx, prem));
        ctx = ctx_next;
        match prem {
            PremInternal::Some(prem) => prems_il.push(prem),
            PremInternal::Var(var_span) => {
                let _ = var_span;
            }
            PremInternal::Else(else_span) => {
                let _ = else_span;
                else_count += 1;
            }
        }
    }
    if else_count > 1 {
        return fail_attempt(
            ElabErrorKind::InvalidPremise,
            span.clone(),
            "cannot use multiple otherwise premises",
        );
    }
    Attempt::ok((ctx, prems_il, else_count == 1))
}

fn elab_not_exp(
    ctx: Context,
    not_typ: &il::NotTyp,
    exp: &el::Exp,
) -> Attempt<(Context, il::NotExp)> {
    if let el::ExpKind::Paren(exp) = &exp.node {
        return elab_not_exp(ctx, not_typ, exp);
    }
    match (&not_typ.node, &exp.node) {
        (Mixfix::Arg(typ), _) => {
            let (ctx, exp) = attempt!(elab_exp(ctx, typ, exp));
            Attempt::ok((ctx, Mixfix::Arg(exp)))
        }
        (Mixfix::Atom(atom_expect), el::ExpKind::Atom(atom)) if atom_expect.node == atom.node => {
            Attempt::ok((ctx, Mixfix::Atom(atom_expect.clone())))
        }
        (Mixfix::Seq(not_typs), el::ExpKind::Seq(exps)) => {
            if not_typs.len() != exps.len() {
                return fail_attempt(
                    ElabErrorKind::NoMatchingAlternative,
                    exp.span.clone(),
                    "notation sequence arity does not match",
                );
            }
            let mut ctx = ctx;
            let mut not_exps = Vec::with_capacity(exps.len());
            for (not_typ_inner, exp) in not_typs.iter().zip(exps) {
                let not_typ_inner = Spanned::new(not_typ_inner.clone(), not_typ.span.clone());
                let (ctx_next, not_exp) = attempt!(elab_not_exp(ctx, &not_typ_inner, exp));
                ctx = ctx_next;
                not_exps.push(not_exp);
            }
            Attempt::ok((ctx, Mixfix::Seq(not_exps)))
        }
        (
            Mixfix::Infix(not_typ_l, atom_expect, not_typ_r),
            el::ExpKind::Infix(exp_l, atom, exp_r),
        ) if atom_expect.node == atom.node => {
            let not_typ_l = Spanned::new((**not_typ_l).clone(), not_typ.span.clone());
            let not_typ_r = Spanned::new((**not_typ_r).clone(), not_typ.span.clone());
            let (ctx, exp_l) = attempt!(elab_not_exp(ctx, &not_typ_l, exp_l));
            let (ctx, exp_r) = attempt!(elab_not_exp(ctx, &not_typ_r, exp_r));
            Attempt::ok((
                ctx,
                Mixfix::Infix(Box::new(exp_l), atom_expect.clone(), Box::new(exp_r)),
            ))
        }
        (
            Mixfix::Brack(atom_expect_l, not_typ_inner, atom_expect_r),
            el::ExpKind::Brack(atom_l, exp_inner, atom_r),
        ) if atom_expect_l.node == atom_l.node && atom_expect_r.node == atom_r.node => {
            let not_typ_inner = Spanned::new((**not_typ_inner).clone(), not_typ.span.clone());
            let (ctx, exp_inner) = attempt!(elab_not_exp(ctx, &not_typ_inner, exp_inner));
            Attempt::ok((
                ctx,
                Mixfix::Brack(
                    atom_expect_l.clone(),
                    Box::new(exp_inner),
                    atom_expect_r.clone(),
                ),
            ))
        }
        _ => fail_attempt(
            ElabErrorKind::NoMatchingAlternative,
            exp.span.clone(),
            "expression does not match notation",
        ),
    }
}

fn elab_struct_exp(
    mut ctx: Context,
    typ_expect: &il::Typ,
    typ_fields: &[il::TypField],
    exp: &el::Exp,
) -> Attempt<(Context, il::Exp)> {
    let el::ExpKind::Str(exp_fields) = &exp.node else {
        return fail_attempt(
            ElabErrorKind::NoMatchingAlternative,
            exp.span.clone(),
            "expression is not a struct",
        );
    };
    if typ_fields.len() != exp_fields.len() {
        return fail_attempt(
            ElabErrorKind::ArityMismatch,
            exp.span.clone(),
            "struct field count does not match",
        );
    }
    let mut fields = Vec::with_capacity(exp_fields.len());
    for ((atom_expect, typ), (atom, exp_field)) in typ_fields.iter().zip(exp_fields) {
        if atom_expect.node != atom.node {
            return fail_attempt(
                ElabErrorKind::TypeMismatch,
                atom.span.clone(),
                "struct field does not match",
            );
        }
        let (ctx_next, exp_field) = attempt!(elab_exp(ctx, typ, exp_field));
        ctx = ctx_next;
        fields.push((atom_expect.clone(), exp_field));
    }
    let exp = Spanned::new(
        Noted::new(il::ExpKind::Str(fields), typ_expect.node.clone()),
        exp.span.clone(),
    );
    Attempt::ok((ctx, exp))
}

fn elab_variant_exp(
    mut ctx: Context,
    typ_expect: &il::Typ,
    typ_cases: &[il::TypCase],
    exp: &el::Exp,
) -> Attempt<(Context, il::Exp)> {
    let mut matches = Vec::new();
    for (not_typ, origin, _) in typ_cases {
        let Attempt::Ok((ctx_next, not_exp)) = elab_not_exp(ctx.clone(), not_typ, exp) else {
            continue;
        };
        let typ_case = Spanned::new(
            il::TypKind::Var(origin.node.0.clone(), origin.node.1.clone()),
            origin.span.clone(),
        );
        let exp_case = Spanned::new(
            Noted::new(il::ExpKind::Case(Box::new(not_exp)), typ_case.node.clone()),
            exp.span.clone(),
        );
        let Attempt::Ok(exp_case) = cast_exp(&ctx_next, typ_expect, &typ_case, exp_case) else {
            continue;
        };
        ctx = ctx_next;
        matches.push(exp_case);
    }
    match matches.len() {
        1 => Attempt::ok((ctx, matches.pop().expect("single variant match"))),
        0 => fail_attempt(
            ElabErrorKind::NoMatchingAlternative,
            exp.span.clone(),
            "expression does not match any variant case",
        ),
        _ => fail_attempt(
            ElabErrorKind::AmbiguousVariant,
            exp.span.clone(),
            "expression matches multiple variant cases",
        ),
    }
}

fn elab_exp_contextual(
    ctx: Context,
    typ_expect: &il::Typ,
    exp: &el::Exp,
) -> Attempt<(Context, il::Exp)> {
    if matches!(&exp.node, el::ExpKind::Var(id) if id.node == "_") {
        let var =
            il_fresh::var_from_typ_wildcard(&ctx.menv, &ctx.frees, exp.span.clone(), typ_expect);
        let exp = il_var::as_exp(false, &var);
        return Attempt::ok((ctx.add_free(var.id), exp));
    }
    if let il::TypKind::Var(id, targs) = &typ_expect.node
        && let Some(TypeDef::Defined(tparams, def_typ)) = ctx.find_typdef_opt(id)
        && let il::DefTypKind::Plain(typ) = &def_typ.node
    {
        let theta = match Theta::from_lists(tparams, targs) {
            Ok(theta) => theta,
            Err(mismatch) => {
                return Attempt::fail(arity_error(
                    mismatch.expected,
                    mismatch.actual,
                    typ_expect.span.clone(),
                ));
            }
        };
        let typ = attempt!(type_result(subst_typ(&theta, typ)));
        return elab_exp_normal(ctx, &typ, exp);
    }
    match &exp.node {
        el::ExpKind::Eps => {
            let (_, iter) = attempt!(as_iter_typ(&ctx, typ_expect));
            let kind = match iter {
                il::Iter::Opt => il::ExpKind::Opt(None),
                il::Iter::List => il::ExpKind::List(vec![]),
            };
            let exp = Spanned::new(Noted::new(kind, typ_expect.node.clone()), exp.span.clone());
            Attempt::ok((ctx, exp))
        }
        el::ExpKind::List(exps) => {
            let (typ_base, iter) = attempt!(as_iter_typ(&ctx, typ_expect));
            if iter != il::Iter::List {
                return fail_attempt(
                    ElabErrorKind::InvalidIteration,
                    exp.span.clone(),
                    "list expression has optional expected type",
                );
            }
            let mut ctx = ctx;
            let mut exps_il = Vec::with_capacity(exps.len());
            for exp in exps {
                let (ctx_next, exp) = attempt!(elab_exp(ctx, &typ_base, exp));
                ctx = ctx_next;
                exps_il.push(exp);
            }
            let exp = Spanned::new(
                Noted::new(il::ExpKind::List(exps_il), typ_expect.node.clone()),
                exp.span.clone(),
            );
            Attempt::ok((ctx, exp))
        }
        el::ExpKind::Cons(exp_head, exp_tail) => {
            let (typ_base, iter) = attempt!(as_iter_typ(&ctx, typ_expect));
            let (ctx, exp_head) = attempt!(elab_exp(ctx, &typ_base, exp_head));
            let typ_tail = Spanned::new(
                il::TypKind::Iter(Box::new(typ_base), iter),
                typ_expect.span.clone(),
            );
            let (ctx, exp_tail) = attempt!(elab_exp(ctx, &typ_tail, exp_tail));
            let kind = il::ExpKind::Cons(Box::new(exp_head), Box::new(exp_tail));
            let exp = Spanned::new(Noted::new(kind, typ_expect.node.clone()), exp.span.clone());
            Attempt::ok((ctx, exp))
        }
        el::ExpKind::Cat(exp_l, exp_r) => {
            let ctx_iter = ctx.clone();
            let typ_iter = typ_expect.clone();
            let exp_l_iter = exp_l.clone();
            let exp_r_iter = exp_r.clone();
            let ctx_text = ctx;
            let exp_l_text = exp_l.clone();
            let exp_r_text = exp_r.clone();
            let (ctx, kind) = attempt!(Attempt::choose_sequential(vec![
                Box::new(move || {
                    let (typ_base, iter) = attempt!(as_iter_typ(&ctx_iter, &typ_iter));
                    let typ_iter = Spanned::new(
                        il::TypKind::Iter(Box::new(typ_base.clone()), iter),
                        typ_base.span,
                    );
                    let (ctx, exp_l) = attempt!(elab_exp(ctx_iter, &typ_iter, &exp_l_iter));
                    let (ctx, exp_r) = attempt!(elab_exp(ctx, &typ_iter, &exp_r_iter));
                    Attempt::ok((ctx, il::ExpKind::Cat(Box::new(exp_l), Box::new(exp_r))))
                }),
                Box::new(move || {
                    let typ_text = typ_at(il::TypKind::Text, &exp_l_text.span);
                    let (ctx, exp_l) = attempt!(elab_exp(ctx_text, &typ_text, &exp_l_text));
                    let typ_text = typ_at(il::TypKind::Text, &exp_r_text.span);
                    let (ctx, exp_r) = attempt!(elab_exp(ctx, &typ_text, &exp_r_text));
                    Attempt::ok((ctx, il::ExpKind::Cat(Box::new(exp_l), Box::new(exp_r))))
                }),
            ]));
            let exp = Spanned::new(Noted::new(kind, typ_expect.node.clone()), exp.span.clone());
            Attempt::ok((ctx, exp))
        }
        el::ExpKind::Tuple(exps) => {
            let typs = attempt!(as_tuple_typ(&ctx, typ_expect));
            if typs.len() != exps.len() {
                return fail_attempt(
                    ElabErrorKind::ArityMismatch,
                    exp.span.clone(),
                    "tuple expression arity does not match",
                );
            }
            let mut ctx = ctx;
            let mut exps_il = Vec::with_capacity(exps.len());
            for (typ, exp) in typs.iter().zip(exps) {
                let (ctx_next, exp) = attempt!(elab_exp(ctx, typ, exp));
                ctx = ctx_next;
                exps_il.push(exp);
            }
            let exp = Spanned::new(
                Noted::new(il::ExpKind::Tuple(exps_il), typ_expect.node.clone()),
                exp.span.clone(),
            );
            Attempt::ok((ctx, exp))
        }
        el::ExpKind::Paren(exp_inner) => {
            let (ctx, exp_inner) = attempt!(elab_exp(ctx, typ_expect, exp_inner));
            let exp = Spanned::new(
                Noted::new(exp_inner.node.kind, exp_inner.node.note),
                exp.span.clone(),
            );
            Attempt::ok((ctx, exp))
        }
        el::ExpKind::Iter(exp_inner, iter) => {
            let (typ_base, iter_expect) = attempt!(as_iter_typ(&ctx, typ_expect));
            let iter = elab_iter(*iter);
            if iter != iter_expect {
                return fail_attempt(
                    ElabErrorKind::InvalidIteration,
                    exp.span.clone(),
                    "iteration mismatch",
                );
            }
            let (ctx, exp_inner) = attempt!(elab_exp(ctx, &typ_base, exp_inner));
            let kind = il::ExpKind::Iter(Box::new(exp_inner), (iter, vec![]));
            let exp = Spanned::new(Noted::new(kind, typ_expect.node.clone()), exp.span.clone());
            Attempt::ok((ctx, exp))
        }
        _ => {
            if let il::TypKind::Var(id, targs) = &typ_expect.node {
                if let Some(TypeDef::Defined(tparams, def_typ)) = ctx.find_typdef_opt(id) {
                    let theta = match Theta::from_lists(tparams, targs) {
                        Ok(theta) => theta,
                        Err(mismatch) => {
                            return Attempt::fail(arity_error(
                                mismatch.expected,
                                mismatch.actual,
                                typ_expect.span.clone(),
                            ));
                        }
                    };
                    if let il::DefTypKind::Struct(fields) = &def_typ.node {
                        let mut fields_subst = Vec::with_capacity(fields.len());
                        for (atom, typ) in fields {
                            let typ = attempt!(type_result(subst_typ(&theta, typ)));
                            fields_subst.push((atom.clone(), typ));
                        }
                        return elab_struct_exp(ctx, typ_expect, &fields_subst, exp);
                    }
                    if let il::DefTypKind::Variant(cases) = &def_typ.node {
                        let mut cases_subst = Vec::with_capacity(cases.len());
                        for case in cases {
                            cases_subst.push(attempt!(type_result(subst_typ_case(&theta, case))));
                        }
                        return elab_variant_exp(ctx, typ_expect, &cases_subst, exp);
                    }
                }
            }
            fail_attempt(
                ElabErrorKind::NoMatchingAlternative,
                exp.span.clone(),
                "expression requires unsupported contextual elaboration",
            )
        }
    }
}

fn valid_tid(id: &Id) -> bool {
    xl::var::strip_var_suffix(id).node == id.node
}

fn distinct_tparams(tparams: &[el::TParam], span: &Span) -> Result<(), ElabError> {
    let mut seen = std::collections::HashSet::new();
    if tparams.iter().all(|tparam| seen.insert(&tparam.node)) {
        Ok(())
    } else {
        Err(ElabError::new(
            ElabErrorKind::Duplicate(EntityKind::Type),
            span.clone(),
            "type parameters are not distinct",
        ))
    }
}

fn fetch_input_hint(
    span: &Span,
    not_typ: &il::NotTyp,
    hints: &[el::Hint],
) -> Result<input::InputHint, ElabError> {
    let arity = not_typ.node.arity();
    let Some((_, hint_exp)) = hints.iter().find(|(id, _)| id.node == "input") else {
        return Ok(input::InputHint::new((0..arity as i64).collect()));
    };
    let Some(input_hint) = input::init(hint_exp) else {
        return Err(ElabError::new(
            ElabErrorKind::InvalidInputHint,
            span.clone(),
            "malformed input hint",
        ));
    };
    input::validate(&input_hint, arity).map_err(|error| {
        ElabError::new(
            ElabErrorKind::InvalidInputHint,
            span.clone(),
            error.to_string(),
        )
    })?;
    Ok(input_hint)
}

fn elab_rule(
    ctx: &Context,
    rule: &el::Rule,
    relid: &Id,
    not_typ: &il::NotTyp,
) -> Result<(il::Rule, bool), ElabError> {
    let (relid_rule, ruleid, exp, prems) = &rule.node;
    if relid_rule.node != relid.node {
        return Err(ElabError::new(
            ElabErrorKind::InvalidRule,
            ruleid.span.clone(),
            "rule relation does not match its group",
        ));
    }
    let mut ctx_local = ctx.clone();
    ctx_local.frees = Default::default();
    ctx_local = ctx_local.add_frees(rule.free());
    let (ctx_local, not_exp) = elab_not_exp(ctx_local, not_typ, exp).commit()?;
    let (_, prems, is_else) = elab_prems(ctx_local, prems, &ruleid.span).commit()?;
    let rule = Spanned::new(
        il::RuleKind {
            id: ruleid.clone(),
            not_exp,
            prems,
        },
        rule.span.clone(),
    );
    Ok((rule, is_else))
}

fn elab_rule_group(
    ctx: &Context,
    span: &Span,
    relid: &Id,
    groupid: &Id,
    rules: &[el::Rule],
) -> Result<(Option<il::RuleGroup>, Option<il::ElseGroup>), ElabError> {
    let (not_typ, _, _, _) = ctx.find_defined_rel(relid)?;
    let not_typ = not_typ.clone();
    let mut rules_il = Vec::with_capacity(rules.len());
    let mut else_rules = Vec::new();
    for rule in rules {
        let (rule, is_else) = elab_rule(ctx, rule, relid, &not_typ)?;
        if is_else {
            else_rules.push(rule);
        } else {
            rules_il.push(rule);
        }
    }
    match else_rules.len() {
        0 => Ok((
            Some(Spanned::new((groupid.clone(), rules_il), span.clone())),
            None,
        )),
        1 if rules.len() == 1 => Ok((
            None,
            Some(Spanned::new(
                (groupid.clone(), else_rules.remove(0)),
                span.clone(),
            )),
        )),
        _ => Err(ElabError::new(
            ElabErrorKind::InvalidRule,
            span.clone(),
            "invalid otherwise rule group",
        )),
    }
}

fn elab_clause(
    ctx: &Context,
    span: &Span,
    id: &Id,
    tparams: &[el::TParam],
    args: &[el::Arg],
    exp: &el::Exp,
    prems: &[el::Prem],
) -> Result<(il::Clause, bool), ElabError> {
    let (tparams_expect, params, typ_ret, _, _) = ctx.find_defined_func(id)?;
    if tparams.len() != tparams_expect.len()
        || tparams
            .iter()
            .zip(tparams_expect)
            .any(|(tparam, expected)| tparam.node != expected.node)
    {
        return Err(ElabError::new(
            ElabErrorKind::ArityMismatch,
            id.span.clone(),
            "type parameters do not match",
        ));
    }
    let params = params.to_vec();
    let typ_ret = typ_ret.clone();
    let mut ctx_local = ctx.clone();
    ctx_local.frees = Default::default();
    let def = Spanned::new(
        el::DefKind::FuncDef(el::FuncDef {
            id: id.clone(),
            tparams: tparams.to_vec(),
            args: args.to_vec(),
            exp: exp.clone(),
            prems: prems.to_vec(),
        }),
        span.clone(),
    );
    ctx_local = ctx_local.add_frees(def.free());
    ctx_local = ctx_local.add_tparams(tparams)?;
    let (ctx_local, args) = elab_args(ctx_local, &params, args, true, span).commit()?;
    let (ctx_local, premises, is_else) = elab_prems(ctx_local, prems, span).commit()?;
    let (_, expression) = elab_exp(ctx_local, &typ_ret, exp).commit()?;
    let clause = Spanned::new(
        il::ClauseKind {
            args,
            expression,
            premises,
        },
        span.clone(),
    );
    Ok((clause, is_else))
}

fn elab_extern_syntax_def(
    mut ctx: Context,
    span: Span,
    def: &el::ExternSyntaxDef,
) -> Result<(Context, il::Def), ElabError> {
    if !valid_tid(&def.id) {
        return Err(ElabError::new(
            ElabErrorKind::InvalidIdentifier,
            def.id.span.clone(),
            "invalid type identifier",
        ));
    }
    ctx = ctx.add_typdef(def.id.clone(), TypeDef::Extern)?;
    let typ = Spanned::new(
        il::TypKind::Var(def.id.clone(), vec![]),
        def.id.span.clone(),
    );
    ctx = ctx.add_metavar(def.id.clone(), typ)?;
    let def_il = Spanned::new(
        il::DefKind::ExternTyp(il::ExternTyp {
            id: def.id.clone(),
            hints: def.hints.clone(),
        }),
        span,
    );
    Ok((ctx, def_il))
}

fn elab_syntax_def(mut ctx: Context, def: &el::SyntaxDef) -> Result<Context, ElabError> {
    for entry in &def.entries {
        distinct_tparams(&entry.tparams, &entry.id.span)?;
        if !valid_tid(&entry.id) {
            return Err(ElabError::new(
                ElabErrorKind::InvalidIdentifier,
                entry.id.span.clone(),
                "invalid type identifier",
            ));
        }
        ctx = ctx.add_typdef(entry.id.clone(), TypeDef::Defining(entry.tparams.clone()))?;
        if entry.tparams.is_empty() {
            let typ = Spanned::new(
                il::TypKind::Var(entry.id.clone(), vec![]),
                entry.id.span.clone(),
            );
            ctx = ctx.add_metavar(entry.id.clone(), typ)?;
        }
    }
    Ok(ctx)
}

fn elab_typ_def(mut ctx: Context, def: &el::TypDef) -> Result<(Context, il::Def), ElabError> {
    match ctx.find_typdef_opt(&def.id) {
        Some(TypeDef::Defining(tparams)) => {
            let matches = tparams.len() == def.tparams.len()
                && tparams
                    .iter()
                    .zip(&def.tparams)
                    .all(|(left, right)| left.node == right.node);
            if !matches {
                return Err(ElabError::new(
                    ElabErrorKind::ArityMismatch,
                    def.id.span.clone(),
                    "type parameters do not match",
                ));
            }
        }
        Some(_) => {
            return Err(ElabError::new(
                ElabErrorKind::Duplicate(EntityKind::Type),
                def.id.span.clone(),
                "type was already defined",
            ));
        }
        None => {
            if !valid_tid(&def.id) || def.tparams.iter().any(|id| !valid_tid(id)) {
                return Err(ElabError::new(
                    ElabErrorKind::InvalidIdentifier,
                    def.id.span.clone(),
                    "invalid type identifier",
                ));
            }
            ctx = ctx.add_typdef(def.id.clone(), TypeDef::Defining(def.tparams.clone()))?;
            if def.tparams.is_empty() {
                let typ = Spanned::new(
                    il::TypKind::Var(def.id.clone(), vec![]),
                    def.id.span.clone(),
                );
                ctx = ctx.add_metavar(def.id.clone(), typ)?;
            }
        }
    }
    let ctx_local = ctx.clone().add_tparams(&def.tparams)?;
    let (typdef, def_typ) = elab_def_typ(&ctx_local, &def.id, &def.tparams, &def.def_typ)?;
    ctx = ctx.update_typdef(&def.id, typdef)?;
    let def_il = Spanned::new(
        il::DefKind::Typ(il::TypDef {
            id: def.id.clone(),
            tparams: def.tparams.clone(),
            def_typ,
            hints: def.hints.clone(),
        }),
        def.def_typ.span.clone(),
    );
    Ok((ctx, def_il))
}

fn elab_var_def(mut ctx: Context, def: &el::VarDef) -> Result<(Context, il::Def), ElabError> {
    if !valid_tid(&def.id) {
        return Err(ElabError::new(
            ElabErrorKind::InvalidIdentifier,
            def.id.span.clone(),
            "invalid meta-variable identifier",
        ));
    }
    if ctx.bound_typdef(&def.id) {
        return Err(ElabError::new(
            ElabErrorKind::Duplicate(EntityKind::Type),
            def.id.span.clone(),
            "type already defined",
        ));
    }
    let typ = elab_plain_typ(&ctx, &def.plain_typ)?;
    ctx = ctx.add_metavar(def.id.clone(), typ.clone())?;
    let def_il = Spanned::new(
        il::DefKind::Var(il::VarDef {
            id: def.id.clone(),
            typ,
            hints: def.hints.clone(),
        }),
        def.id.span.clone(),
    );
    Ok((ctx, def_il))
}

fn elab_rel_def(
    mut ctx: Context,
    span: Span,
    id: &Id,
    not_typ: &el::NotTyp,
    hints: &[el::Hint],
    is_extern: bool,
) -> Result<(Context, il::Def), ElabError> {
    let not_typ = elab_not_typ(&ctx, &el::Typ::Notation(not_typ.clone()))?;
    let input_hint = fetch_input_hint(&span, &not_typ, hints)?;
    let kind = if is_extern {
        ctx = ctx.add_extern_rel(id.clone(), not_typ.clone(), input_hint.clone())?;
        il::DefKind::ExternRel(il::ExternRel {
            id: id.clone(),
            not_typ,
            input_hint,
            hints: hints.to_vec(),
        })
    } else {
        ctx = ctx.add_defined_rel(id.clone(), not_typ.clone(), input_hint.clone())?;
        il::DefKind::Rel(il::Rel {
            id: id.clone(),
            not_typ,
            input_hint,
            rule_groups: vec![],
            else_group: None,
            hints: hints.to_vec(),
        })
    };
    Ok((ctx, Spanned::new(kind, span)))
}

#[derive(Clone, Copy)]
enum DecKind {
    Extern,
    Builtin,
    Defined,
}

fn elab_func_dec(
    mut ctx: Context,
    span: Span,
    id: &Id,
    tparams: &[el::TParam],
    params: &[el::Param],
    plain_typ: &el::PlainTyp,
    hints: &[el::Hint],
    kind: DecKind,
) -> Result<(Context, il::Def), ElabError> {
    distinct_tparams(tparams, &id.span)?;
    let ctx_local = ctx.clone().add_tparams(tparams)?;
    let params_il = params
        .iter()
        .map(|param| elab_param(&ctx_local, param))
        .collect::<Result<Vec<_>, _>>()?;
    let typ = elab_plain_typ(&ctx_local, plain_typ)?;
    let def_kind = match kind {
        DecKind::Extern => {
            ctx =
                ctx.add_extern_func(id.clone(), tparams.to_vec(), params_il.clone(), typ.clone())?;
            il::DefKind::ExternDec(il::ExternDec {
                id: id.clone(),
                tparams: tparams.to_vec(),
                params: params_il,
                typ,
                hints: hints.to_vec(),
            })
        }
        DecKind::Builtin => {
            ctx =
                ctx.add_builtin_func(id.clone(), tparams.to_vec(), params_il.clone(), typ.clone())?;
            il::DefKind::BuiltinDec(il::BuiltinDec {
                id: id.clone(),
                tparams: tparams.to_vec(),
                params: params_il,
                typ,
                hints: hints.to_vec(),
            })
        }
        DecKind::Defined => {
            ctx =
                ctx.add_defined_func(id.clone(), tparams.to_vec(), params_il.clone(), typ.clone())?;
            il::DefKind::FuncDec(il::FuncDec {
                id: id.clone(),
                tparams: tparams.to_vec(),
                params: params_il,
                typ,
                clauses: vec![],
                else_clause: None,
                hints: hints.to_vec(),
            })
        }
    };
    Ok((ctx, Spanned::new(def_kind, span)))
}

fn elab_table_dec(
    mut ctx: Context,
    span: Span,
    def: &el::TableDecDef,
) -> Result<(Context, il::Def), ElabError> {
    let params = def
        .params
        .iter()
        .map(|param| elab_param(&ctx, param))
        .collect::<Result<Vec<_>, _>>()?;
    if params
        .iter()
        .any(|param| !matches!(param.node, il::ParamKind::Exp(_)))
    {
        return Err(ElabError::new(
            ElabErrorKind::InvalidDefinition,
            span,
            "table cannot have function parameters",
        ));
    }
    let typ = elab_plain_typ(&ctx, &def.plain_typ)?;
    if typ.node != il::TypKind::Bool {
        return Err(ElabError::new(
            ElabErrorKind::TypeMismatch,
            typ.span,
            "table must return boolean",
        ));
    }
    ctx = ctx.add_table_func(def.id.clone(), params.clone(), typ.clone())?;
    let def_il = Spanned::new(
        il::DefKind::TableDec(il::TableDec {
            id: def.id.clone(),
            params,
            typ,
            rows: vec![],
            hints: def.hints.clone(),
        }),
        span,
    );
    Ok((ctx, def_il))
}

fn elab_table_def(mut ctx: Context, def: &el::TableDef) -> Result<Context, ElabError> {
    let (params, typ, _) = ctx.find_table_func(&def.id)?;
    let params = params.to_vec();
    let typ = typ.clone();
    let mut rows = Vec::with_capacity(def.rows.len());
    for row in &def.rows {
        let (exp_pattern, exp_body) = &row.node;
        let exps = match &exp_pattern.node {
            el::ExpKind::Tuple(exps) => exps.clone(),
            _ => vec![exp_pattern.clone()],
        };
        let args = exps
            .into_iter()
            .map(|exp| {
                let span = exp.span.clone();
                Spanned::new(el::ArgKind::Exp(Box::new(exp)), span)
            })
            .collect::<Vec<_>>();
        let mut ctx_local = ctx.clone();
        ctx_local.frees = Default::default();
        ctx_local = ctx_local.add_frees(row.free());
        let (ctx_local, args) = elab_args(ctx_local, &params, &args, true, &row.span).commit()?;
        let (_, exp_body) = elab_exp(ctx_local, &typ, exp_body).commit()?;
        rows.push(Spanned::new((args, exp_body), row.span.clone()));
    }
    ctx = ctx.add_table_func_rows(&def.id, rows)?;
    Ok(ctx)
}

fn elab_def(ctx: Context, def: &el::Def) -> Result<(Context, Option<il::Def>), ElabError> {
    let span = def.span.clone();
    match &def.node {
        el::DefKind::ExternSyntax(def) => {
            let (ctx, def) = elab_extern_syntax_def(ctx, span, def)?;
            Ok((ctx, Some(def)))
        }
        el::DefKind::Syntax(def) => Ok((elab_syntax_def(ctx, def)?, None)),
        el::DefKind::Typ(def) => {
            let (ctx, def) = elab_typ_def(ctx, def)?;
            Ok((ctx, Some(def)))
        }
        el::DefKind::Var(def) => {
            let (ctx, def) = elab_var_def(ctx, def)?;
            Ok((ctx, Some(def)))
        }
        el::DefKind::ExternRel(def) => {
            let (ctx, def) = elab_rel_def(ctx, span, &def.id, &def.not_typ, &def.hints, true)?;
            Ok((ctx, Some(def)))
        }
        el::DefKind::Rel(def) => {
            let (ctx, def) = elab_rel_def(ctx, span, &def.id, &def.not_typ, &def.hints, false)?;
            Ok((ctx, Some(def)))
        }
        el::DefKind::RuleGroup(def) => {
            let (rule_group, else_group) =
                elab_rule_group(&ctx, &span, &def.relid, &def.groupid, &def.rules)?;
            let ctx = if let Some(rule_group) = rule_group {
                ctx.add_defined_rule_group(&def.relid, rule_group)?
            } else {
                ctx
            };
            let ctx = if let Some(else_group) = else_group {
                ctx.add_defined_else_group(&def.relid, else_group)?
            } else {
                ctx
            };
            Ok((ctx, None))
        }
        el::DefKind::ExternDec(def) => {
            let (ctx, def) = elab_func_dec(
                ctx,
                span,
                &def.id,
                &def.tparams,
                &def.params,
                &def.plain_typ,
                &def.hints,
                DecKind::Extern,
            )?;
            Ok((ctx, Some(def)))
        }
        el::DefKind::BuiltinDec(def) => {
            let (ctx, def) = elab_func_dec(
                ctx,
                span,
                &def.id,
                &def.tparams,
                &def.params,
                &def.plain_typ,
                &def.hints,
                DecKind::Builtin,
            )?;
            Ok((ctx, Some(def)))
        }
        el::DefKind::TableDec(def) => {
            let (ctx, def) = elab_table_dec(ctx, span, def)?;
            Ok((ctx, Some(def)))
        }
        el::DefKind::FuncDec(def) => {
            let (ctx, def) = elab_func_dec(
                ctx,
                span,
                &def.id,
                &def.tparams,
                &def.params,
                &def.plain_typ,
                &def.hints,
                DecKind::Defined,
            )?;
            Ok((ctx, Some(def)))
        }
        el::DefKind::TableDef(def) => Ok((elab_table_def(ctx, def)?, None)),
        el::DefKind::FuncDef(def) => {
            let (clause, is_else) = elab_clause(
                &ctx,
                &span,
                &def.id,
                &def.tparams,
                &def.args,
                &def.exp,
                &def.prems,
            )?;
            let ctx = if is_else {
                ctx.add_defined_func_else_clause(&def.id, clause)?
            } else {
                ctx.add_defined_func_clause(&def.id, clause)?
            };
            Ok((ctx, None))
        }
        el::DefKind::Sep => Ok((ctx, None)),
    }
}

fn populate_defs(ctx: &Context, defs: il::Spec) -> Result<il::Spec, ElabError> {
    defs.into_iter()
        .map(|def| {
            let kind = match def.node {
                il::DefKind::Rel(mut rel) => {
                    if !rel.rule_groups.is_empty() || rel.else_group.is_some() {
                        return Err(ElabError::new(
                            ElabErrorKind::AlreadyPopulated,
                            def.span,
                            "relation was already populated",
                        ));
                    }
                    let (_, _, groups, else_group) = ctx.find_defined_rel(&rel.id)?;
                    rel.rule_groups = groups.to_vec();
                    rel.else_group = else_group.cloned();
                    il::DefKind::Rel(rel)
                }
                il::DefKind::TableDec(mut table) => {
                    if !table.rows.is_empty() {
                        return Err(ElabError::new(
                            ElabErrorKind::AlreadyPopulated,
                            def.span,
                            "table was already populated",
                        ));
                    }
                    let (_, _, rows) = ctx.find_table_func(&table.id)?;
                    table.rows = rows.to_vec();
                    il::DefKind::TableDec(table)
                }
                il::DefKind::FuncDec(mut func) => {
                    if !func.clauses.is_empty() || func.else_clause.is_some() {
                        return Err(ElabError::new(
                            ElabErrorKind::AlreadyPopulated,
                            def.span,
                            "function was already populated",
                        ));
                    }
                    let (_, _, _, clauses, else_clause) = ctx.find_defined_func(&func.id)?;
                    func.clauses = clauses.to_vec();
                    func.else_clause = else_clause.cloned();
                    il::DefKind::FuncDec(func)
                }
                kind => kind,
            };
            Ok(Spanned::new(kind, def.span))
        })
        .collect()
}

pub(super) fn elaborate(spec: &el::Spec) -> Result<il::Spec, ElabError> {
    let mut ctx = Context::new();
    let mut defs = Vec::new();
    for def in spec {
        let (ctx_next, def) = elab_def(ctx, def)?;
        ctx = ctx_next;
        if let Some(def) = def {
            defs.push(def);
        }
    }
    let defs = populate_defs(&ctx, defs)?;
    dimension::analyze_spec(&defs)
}

#[cfg(test)]
mod tests {
    use crate::{
        lang::{
            common::{
                notation::{atom::Atom, mixfix::Mixfix},
                source::{Position, Span, Spanned},
            },
            el::ast as el,
            il::ast::{self as il, DefTypKind, TypKind},
        },
        pass::elaborate::{ElabErrorKind, EntityKind},
        runtime::types::TypeDef,
    };

    use super::{super::context::Context, elab_def_typ, elab_exp, elab_plain_typ, infer_exp};

    fn span(label: &str) -> Span {
        Span::new(Position::new(label, 4, 2), Position::new(label, 4, 6))
    }

    fn id(name: &str, label: &str) -> el::Id {
        Spanned::new(name.to_owned(), span(label))
    }

    fn plain(kind: el::PlainTypKind, label: &str) -> el::PlainTyp {
        Spanned::new(kind, span(label))
    }

    fn targ(kind: el::PlainTypKind, label: &str) -> el::Targ {
        Spanned::new(kind, span(label))
    }

    fn atom(name: &str, label: &str) -> el::Atom {
        Spanned::new(Atom::keyword(name), span(label))
    }

    fn exp(kind: el::ExpKind, label: &str) -> el::Exp {
        Spanned::new(kind, span(label))
    }

    #[test]
    fn plain_type_elaboration_preserves_arguments_and_source_spans() {
        let alias = id("Alias", "alias-definition");
        let tparam = id("T", "parameter");
        let alias_def = Spanned::new(
            DefTypKind::Plain(crate::runtime::types::typ::bool()),
            span("alias-body"),
        );
        let ctx = Context::new()
            .add_typdef(
                alias.clone(),
                TypeDef::Defined(vec![tparam], Box::new(alias_def)),
            )
            .expect("alias definition");
        let input_span = span("alias-use");
        let input = Spanned::new(
            el::PlainTypKind::Var(alias.clone(), vec![targ(el::PlainTypKind::Bool, "arg")]),
            input_span.clone(),
        );

        let output = elab_plain_typ(&ctx, &input).expect("elaborate type");

        assert_eq!(output.span, input_span);
        assert!(matches!(
            output.node,
            TypKind::Var(id, targs)
                if id == alias && matches!(targs.as_slice(), [arg] if arg.node == TypKind::Bool)
        ));
    }

    #[test]
    fn plain_type_elaboration_reports_lookup_and_arity_categories() {
        let missing_span = span("missing-use");
        let missing = plain(
            el::PlainTypKind::Var(id("Missing", "missing-use"), vec![]),
            "missing-use",
        );
        let error = elab_plain_typ(&Context::new(), &missing).unwrap_err();
        assert_eq!(error.kind, ElabErrorKind::Undefined(EntityKind::Type));
        assert_eq!(error.span, missing_span);

        let alias = id("Alias", "alias-definition");
        let ctx = Context::new()
            .add_typdef(alias.clone(), TypeDef::Defining(vec![id("T", "parameter")]))
            .expect("alias declaration");
        let wrong_arity = plain(el::PlainTypKind::Var(alias, vec![]), "wrong-arity");
        let error = elab_plain_typ(&ctx, &wrong_arity).unwrap_err();
        assert_eq!(error.kind, ElabErrorKind::ArityMismatch);
        assert_eq!(error.span, span("alias-definition"));
    }

    #[test]
    fn variant_extension_substitutes_arguments_and_preserves_origin() {
        let base = id("Base", "base");
        let tparam = id("T", "base-parameter");
        let not_typ = Spanned::new(
            Mixfix::Seq(vec![
                Mixfix::Atom(atom("CASE", "case")),
                Mixfix::Arg(crate::runtime::types::typ::var(tparam.clone(), vec![])),
            ]),
            span("case"),
        );
        let origin = Spanned::new(
            (
                base.clone(),
                vec![crate::runtime::types::typ::var(tparam.clone(), vec![])],
            ),
            span("base"),
        );
        let base_def = Spanned::new(
            DefTypKind::Variant(vec![(not_typ, origin, vec![])]),
            span("base-body"),
        );
        let ctx = Context::new()
            .add_typdef(
                base.clone(),
                TypeDef::Defined(vec![tparam], Box::new(base_def)),
            )
            .expect("base definition");
        let extension = Spanned::new(
            el::DefTypKind::Variant(vec![(
                el::Typ::Plain(plain(
                    el::PlainTypKind::Var(
                        base.clone(),
                        vec![targ(el::PlainTypKind::Bool, "argument")],
                    ),
                    "extension",
                )),
                vec![],
            )]),
            span("extension"),
        );

        let (_, output) =
            elab_def_typ(&ctx, &id("Derived", "derived"), &[], &extension).expect("extend variant");

        let DefTypKind::Variant(cases) = output.node else {
            panic!("variant definition")
        };
        let (not_typ, origin, _) = &cases[0];
        assert!(matches!(
            &not_typ.node,
            Mixfix::Seq(items)
                if matches!(&items[1], Mixfix::Arg(typ) if typ.node == TypKind::Bool)
        ));
        assert_eq!(origin.node.0, base);
    }

    #[test]
    fn variant_definition_rejects_duplicate_notation_shapes() {
        let case = |typ, label| {
            (
                el::Typ::Notation(Spanned::new(
                    el::NotTypKind::Seq(vec![
                        el::Typ::Notation(Spanned::new(
                            el::NotTypKind::Atom(atom("CASE", label)),
                            span(label),
                        )),
                        el::Typ::Plain(plain(typ, label)),
                    ]),
                    span(label),
                )),
                vec![],
            )
        };
        let definition = Spanned::new(
            el::DefTypKind::Variant(vec![
                case(el::PlainTypKind::Bool, "bool-case"),
                case(el::PlainTypKind::Text, "text-case"),
            ]),
            span("variant"),
        );

        let error =
            elab_def_typ(&Context::new(), &id("Choice", "choice"), &[], &definition).unwrap_err();

        assert_eq!(error.kind, ElabErrorKind::AmbiguousVariant);
        assert_eq!(error.span, span("variant"));
    }

    #[test]
    fn inference_uses_metavariable_base_names_and_preserves_occurrence_ids() {
        let ctx = Context::new()
            .add_metavar(id("item", "binding"), crate::runtime::types::typ::text())
            .expect("metavariable");
        let occurrence = id("item_suffix", "occurrence");
        let input = exp(el::ExpKind::Var(occurrence.clone()), "occurrence");

        let (_, output, typ) = infer_exp(ctx, &input).commit().expect("infer variable");

        assert_eq!(typ.node, TypKind::Text);
        assert!(matches!(output.node.kind, il::ExpKind::Var(id) if id == occurrence));
        assert_eq!(output.span, span("occurrence"));
    }

    #[test]
    fn checking_inserts_the_numeric_upcast_after_inference() {
        let number = crate::lang::xl::num::Number::Nat(1_u64.into());
        let input = exp(el::ExpKind::Num(el::NumOp::Dec, number), "number");
        let expected = Spanned::new(
            TypKind::Num(crate::lang::xl::num::Typ::Int),
            span("expected"),
        );

        let (_, output) = elab_exp(Context::new(), &expected, &input)
            .commit()
            .expect("cast number");

        assert!(matches!(
            output.node.kind,
            il::ExpKind::UpCast(typ, _) if typ.node == TypKind::Num(crate::lang::xl::num::Typ::Int)
        ));
        assert_eq!(
            output.node.note,
            TypKind::Num(crate::lang::xl::num::Typ::Int)
        );
    }

    #[test]
    fn parenthesized_casts_keep_the_parent_span_through_the_operand() {
        let number = crate::lang::xl::num::Number::Nat(1_u64.into());
        let inner = exp(el::ExpKind::Num(el::NumOp::Dec, number), "number");
        let input = exp(el::ExpKind::Paren(Box::new(inner)), "parentheses");
        let expected = Spanned::new(
            TypKind::Num(crate::lang::xl::num::Typ::Int),
            span("expected"),
        );

        let (_, output) = elab_exp(Context::new(), &expected, &input)
            .commit()
            .expect("cast parenthesized number");
        let il::ExpKind::UpCast(_, operand) = &output.node.kind else {
            panic!("expected numeric upcast")
        };

        assert_eq!(output.span, span("parentheses"));
        assert_eq!(operand.span, span("parentheses"));
    }

    #[test]
    fn plain_aliases_are_unwrapped_before_contextual_elaboration() {
        let alias = id("OptionalText", "alias");
        let element = Spanned::new(TypKind::Text, span("element"));
        let optional = Spanned::new(
            TypKind::Iter(Box::new(element), il::Iter::Opt),
            span("optional"),
        );
        let definition = Spanned::new(DefTypKind::Plain(optional), span("definition"));
        let ctx = Context::new()
            .add_typdef(
                alias.clone(),
                TypeDef::Defined(vec![], Box::new(definition)),
            )
            .expect("alias definition");
        let expected = Spanned::new(TypKind::Var(alias, vec![]), span("expected"));
        let input = exp(el::ExpKind::Eps, "epsilon");

        let (_, output) = elab_exp(ctx, &expected, &input)
            .commit()
            .expect("elaborate empty optional alias");

        assert!(matches!(output.node.kind, il::ExpKind::Opt(None)));
        assert!(matches!(
            output.node.note,
            TypKind::Iter(typ, il::Iter::Opt)
                if typ.node == TypKind::Text && typ.span == span("element")
        ));
    }

    #[test]
    fn inference_rejects_heterogeneous_list_elements_at_the_list_span() {
        let list_span = span("list");
        let input = Spanned::new(
            el::ExpKind::List(vec![
                exp(el::ExpKind::Bool(true), "bool"),
                exp(el::ExpKind::Text("text".to_owned()), "text"),
            ]),
            list_span.clone(),
        );

        let error = infer_exp(Context::new(), &input).commit().unwrap_err();

        assert_eq!(error.kind, ElabErrorKind::CannotInfer);
        assert_eq!(error.span, list_span);
    }
}
