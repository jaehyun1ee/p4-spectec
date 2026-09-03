//! Elaboration-language validation and conversion to intermediate syntax

use std::borrow::Cow;

use crate::{
    lang::{
        common::{
            Id,
            ds::map::ArityMismatch,
            notation::mixfix::Mixfix,
            source::{Phrase, Span},
        },
        el::ast as el,
        hints::input,
        il::{ast as il, fresh as il_fresh, var as il_var},
        traits::free::Free,
        xl,
    },
    note_phrase, phrase,
    runtime::{
        sta::{Func, Rel},
        types::{
            Theta, TypeArityMismatch, TypeDef, TypeErrorKind, equiv_func_typ, equiv_typ,
            expand_typ, optimize_sub_typ, sub_typ, subst_not_typ, subst_params, subst_typ,
            subst_typs,
        },
    },
};

use super::{
    ElabError, ElabErrorKind, EntityKind, TypeShape,
    attempt::{Attempt, choose_sequential, fail, fail_silent, finish},
    context::Context,
    dimension,
};

// == Checks

// - Identifiers

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

// == Iterations

// - Iteration elaboration

fn elab_iter(iter: el::Iter) -> il::Iter {
    match iter {
        el::Iter::Opt => il::Iter::Opt,
        el::Iter::List => il::Iter::List,
    }
}

// == Types

// - Type destructuring

fn destruct_error(shape: TypeShape, span: Span) -> ElabError {
    ElabError::new(
        ElabErrorKind::CannotDestructure(shape),
        span,
        format!("cannot destruct type as {shape}"),
    )
}

fn as_text_typ(ctx: &Context, typ_il: &il::Typ) -> Attempt<()> {
    let typ_il = expand_typ(&ctx.tdenv, typ_il)?;
    match &typ_il.node {
        il::TypKind::Text => Ok(()),
        _ => fail(destruct_error(TypeShape::Text, typ_il.span.clone())),
    }
}

fn as_iter_typ(ctx: &Context, typ_il: &il::Typ) -> Attempt<(il::Typ, il::Iter)> {
    let typ_il = expand_typ(&ctx.tdenv, typ_il)?;
    match typ_il {
        Cow::Borrowed(typ_il) => {
            let il::TypKind::Iter(typ_inner_il, iter_il) = &typ_il.node else {
                return fail(destruct_error(TypeShape::Iteration, typ_il.span.clone()));
            };
            Ok(((**typ_inner_il).clone(), *iter_il))
        }
        Cow::Owned(typ_il) => {
            let span = typ_il.span;
            let il::TypKind::Iter(typ_inner_il, iter_il) = typ_il.node else {
                return fail(destruct_error(TypeShape::Iteration, span));
            };
            Ok((*typ_inner_il, iter_il))
        }
    }
}

fn as_tuple_typ(ctx: &Context, typ_il: &il::Typ) -> Attempt<Vec<il::Typ>> {
    let typ_il = expand_typ(&ctx.tdenv, typ_il)?;
    match typ_il {
        Cow::Borrowed(typ_il) => {
            let il::TypKind::Tuple(typs_il) = &typ_il.node else {
                return fail(destruct_error(TypeShape::Tuple, typ_il.span.clone()));
            };
            Ok(typs_il.clone())
        }
        Cow::Owned(typ_il) => {
            let span = typ_il.span;
            let il::TypKind::Tuple(typs_il) = typ_il.node else {
                return fail(destruct_error(TypeShape::Tuple, span));
            };
            Ok(typs_il)
        }
    }
}

fn as_list_typ(ctx: &Context, typ_il: &il::Typ) -> Attempt<il::Typ> {
    let typ_il = expand_typ(&ctx.tdenv, typ_il)?;
    match typ_il {
        Cow::Borrowed(typ_il) => {
            let il::TypKind::Iter(typ_inner_il, il::Iter::List) = &typ_il.node else {
                return fail(destruct_error(TypeShape::List, typ_il.span.clone()));
            };
            Ok((**typ_inner_il).clone())
        }
        Cow::Owned(typ_il) => {
            let span = typ_il.span;
            let il::TypKind::Iter(typ_inner_il, il::Iter::List) = typ_il.node else {
                return fail(destruct_error(TypeShape::List, span));
            };
            Ok(*typ_inner_il)
        }
    }
}

fn as_struct_typ(ctx: &Context, typ_il: &il::Typ) -> Attempt<Vec<il::TypField>> {
    let typ_il = expand_typ(&ctx.tdenv, typ_il)?;
    let il::TypKind::Var(id, _) = &typ_il.node else {
        return fail(destruct_error(TypeShape::Struct, typ_il.span.clone()));
    };
    let Some(TypeDef::Defined(_, def_typ_il)) = ctx.find_typdef_opt(id) else {
        return fail(destruct_error(TypeShape::Struct, typ_il.span.clone()));
    };
    match &def_typ_il.node {
        il::DefTypKind::Struct(typ_fields_il) => Ok(typ_fields_il.clone()),
        _ => fail(destruct_error(TypeShape::Struct, typ_il.span.clone())),
    }
}

// - Plain types

fn arity_error(expected: usize, actual: usize, span: Span) -> ElabError {
    let mismatch = ArityMismatch::new(expected, actual);
    let mismatch = TypeArityMismatch::TypeArgument(mismatch);
    let type_error = TypeErrorKind::ArityMismatch(mismatch);
    ElabError::new(ElabErrorKind::ArityMismatch, span, type_error.to_string())
}

fn elab_plain_typ(ctx: &Context, plain_typ: &el::PlainTyp) -> Result<il::Typ, ElabError> {
    let typ_il_kind = match &plain_typ.node {
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
                let targ_il = elab_plain_typ(ctx, targ)?;
                targs_il.push(targ_il);
            }
            il::TypKind::Var(id.clone(), targs_il)
        }
        el::PlainTypKind::Paren(plain_typ) => {
            let typ_il = elab_plain_typ(ctx, plain_typ)?;
            typ_il.node
        }
        el::PlainTypKind::Tuple(plain_typs) => {
            let mut typs_il = Vec::with_capacity(plain_typs.len());
            for plain_typ in plain_typs {
                let typ_il = elab_plain_typ(ctx, plain_typ)?;
                typs_il.push(typ_il);
            }
            il::TypKind::Tuple(typs_il)
        }
        el::PlainTypKind::Iter(plain_typ, iter) => {
            let typ_il = elab_plain_typ(ctx, plain_typ)?;
            il::TypKind::Iter(Box::new(typ_il), elab_iter(*iter))
        }
    };
    let typ_il = phrase!(node: typ_il_kind, span: plain_typ.span.clone());
    Ok(typ_il)
}

// - Notation types

fn elab_not_typ(ctx: &Context, typ: &el::Typ) -> Result<il::NotTyp, ElabError> {
    match typ {
        el::Typ::Plain(plain_typ) => {
            let typ_il = elab_plain_typ(ctx, plain_typ)?;
            let not_typ_il = Mixfix::Arg(typ_il);
            let not_typ_il = phrase!(node: not_typ_il, span: plain_typ.span.clone());
            Ok(not_typ_il)
        }
        el::Typ::Notation(not_typ) => {
            let not_typ_il_kind = match &not_typ.node {
                el::NotTypKind::Atom(atom) => Mixfix::Atom(atom.clone()),
                el::NotTypKind::Seq(typs) => {
                    let mut not_typs_il = Vec::with_capacity(typs.len());
                    for typ in typs {
                        let not_typ_il = elab_not_typ(ctx, typ)?;
                        not_typs_il.push(not_typ_il.node);
                    }
                    Mixfix::Seq(not_typs_il)
                }
                el::NotTypKind::Infix(typ_l, atom, typ_r) => {
                    let not_typ_il_l = elab_not_typ(ctx, typ_l)?;
                    let not_typ_il_r = elab_not_typ(ctx, typ_r)?;
                    Mixfix::Infix(
                        Box::new(not_typ_il_l.node),
                        atom.clone(),
                        Box::new(not_typ_il_r.node),
                    )
                }
                el::NotTypKind::Brack(atom_l, typ, atom_r) => {
                    let not_typ_il = elab_not_typ(ctx, typ)?;
                    Mixfix::Brack(atom_l.clone(), Box::new(not_typ_il.node), atom_r.clone())
                }
            };
            let not_typ_il = phrase!(node: not_typ_il_kind, span: not_typ.span.clone());
            Ok(not_typ_il)
        }
    }
}

// - Definition types

fn elab_typ_case_plain(ctx: &Context, typ_il: &il::Typ) -> Result<Vec<il::TypCase>, ElabError> {
    let typ_il = expand_typ(&ctx.tdenv, typ_il)?;
    let il::TypKind::Var(id, targs_il) = &typ_il.node else {
        return Err(ElabError::new(
            ElabErrorKind::InvalidTypeExtension,
            typ_il.span.clone(),
            "cannot extend a non-variant type",
        ));
    };
    match ctx.find_typdef(id)? {
        TypeDef::Defining(_) => Err(ElabError::new(
            ElabErrorKind::InvalidTypeExtension,
            typ_il.span.clone(),
            "cannot extend an incomplete type",
        )),
        TypeDef::Defined(tparams, def_typ_il) => {
            let il::DefTypKind::Variant(typ_cases_il) = &def_typ_il.node else {
                return Err(ElabError::new(
                    ElabErrorKind::InvalidTypeExtension,
                    typ_il.span.clone(),
                    "cannot extend a non-variant type",
                ));
            };
            let theta = Theta::from_lists(tparams, targs_il).map_err(|mismatch| {
                arity_error(mismatch.expected, mismatch.actual, typ_il.span.clone())
            })?;
            typ_cases_il
                .iter()
                .map(|(not_typ_il, origin_il, hints)| {
                    let not_typ_il = subst_not_typ(&theta, not_typ_il).map_err(ElabError::from)?;
                    let targs_il =
                        subst_typs(&theta, &origin_il.node.1).map_err(ElabError::from)?;
                    let origin_il = phrase! {
                        node: (origin_il.node.0.clone(), targs_il),
                        span: origin_il.span.clone(),
                    };
                    Ok((not_typ_il, origin_il, hints.clone()))
                })
                .collect()
        }
        TypeDef::Parameter | TypeDef::Extern => Err(ElabError::new(
            ElabErrorKind::InvalidTypeExtension,
            typ_il.span.clone(),
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
            let typ_il = elab_plain_typ(ctx, plain_typ)?;
            let def_typ_il_kind = il::DefTypKind::Plain(typ_il);
            phrase!(node: def_typ_il_kind, span: plain_typ.span.clone())
        }
        el::DefTypKind::Struct(fields) => {
            let mut typ_fields_il = Vec::with_capacity(fields.len());
            for (atom, plain_typ, _) in fields {
                let typ_il = elab_plain_typ(ctx, plain_typ)?;
                typ_fields_il.push((atom.clone(), typ_il));
            }
            let def_typ_il_kind = il::DefTypKind::Struct(typ_fields_il);
            phrase!(node: def_typ_il_kind, span: def_typ.span.clone())
        }
        el::DefTypKind::Variant(cases) => {
            let targs_il = tparams
                .iter()
                .map(|tparam| {
                    let typ_il_kind = il::TypKind::Var(tparam.clone(), vec![]);
                    phrase!(node: typ_il_kind, span: tparam.span.clone())
                })
                .collect();
            let origin_il_node = (id.clone(), targs_il);
            let origin_il = phrase!(node: origin_il_node, span: id.span.clone());
            let mut typ_cases_il = vec![];
            for (typ, hints) in cases {
                match typ {
                    el::Typ::Plain(plain_typ) => {
                        let typ_il = elab_plain_typ(ctx, plain_typ)?;
                        let typ_cases_il_plain = elab_typ_case_plain(ctx, &typ_il)?;
                        typ_cases_il.extend(typ_cases_il_plain);
                    }
                    el::Typ::Notation(_) => {
                        let not_typ_il = elab_not_typ(ctx, typ)?;
                        typ_cases_il.push((not_typ_il, origin_il.clone(), hints.clone()));
                    }
                }
            }
            for (index, typ_case_il) in typ_cases_il.iter().enumerate() {
                let mixop = typ_case_il.0.node.to_mixop();
                if typ_cases_il[..index]
                    .iter()
                    .any(|typ_case_il_other| typ_case_il_other.0.node.to_mixop() == mixop)
                {
                    return Err(ElabError::new(
                        ElabErrorKind::AmbiguousVariant,
                        def_typ.span.clone(),
                        "variant cases are ambiguous",
                    ));
                }
            }
            let def_typ_il_kind = il::DefTypKind::Variant(typ_cases_il);
            phrase!(node: def_typ_il_kind, span: def_typ.span.clone())
        }
    };
    let type_def = TypeDef::Defined(tparams.to_vec(), Box::new(def_typ_il.clone()));
    Ok((type_def, def_typ_il))
}

// == Elaboration helpers

fn fail_attempt<T>(kind: ElabErrorKind, span: Span, message: impl Into<String>) -> Attempt<T> {
    fail(ElabError::new(kind, span, message))
}

fn typ_at(typ_il_kind: il::TypKind, span: &Span) -> il::Typ {
    phrase!(node: typ_il_kind, span: span.clone())
}

// == Expressions

// - Expression type inference

fn fail_infer<T>(span: &Span, construct: &str) -> Attempt<T> {
    fail_attempt(
        ElabErrorKind::CannotInfer,
        span.clone(),
        format!("cannot infer type of {construct}"),
    )
}

fn infer_exp(ctx: &mut Context, exp: &el::Exp) -> Attempt<il::Exp> {
    match &exp.node {
        el::ExpKind::Bool(value) => infer_bool_exp(ctx, &exp.span, *value),
        el::ExpKind::Num(_, value) => infer_num_exp(ctx, &exp.span, value),
        el::ExpKind::Text(value) => infer_text_exp(ctx, &exp.span, value),
        el::ExpKind::Var(id) => infer_var_exp(ctx, &exp.span, id),
        el::ExpKind::Un(op, exp_inner) => infer_un_exp(ctx, &exp.span, *op, exp_inner),
        el::ExpKind::Bin(exp_l, op, exp_r) => infer_bin_exp(ctx, &exp.span, exp_l, *op, exp_r),
        el::ExpKind::Cmp(exp_l, op, exp_r) => infer_cmp_exp(ctx, &exp.span, exp_l, *op, exp_r),
        el::ExpKind::Arith(exp_inner) => infer_arith_exp(ctx, &exp.span, exp_inner),
        el::ExpKind::List(exps) => infer_list_exp(ctx, &exp.span, exps),
        el::ExpKind::Cons(exp_head, exp_tail) => infer_cons_exp(ctx, &exp.span, exp_head, exp_tail),
        el::ExpKind::Cat(exp_l, exp_r) => infer_cat_exp(ctx, &exp.span, exp_l, exp_r),
        el::ExpKind::Idx(exp_base, exp_index) => infer_idx_exp(ctx, &exp.span, exp_base, exp_index),
        el::ExpKind::Slice(exp_base, exp_index, exp_length) => {
            infer_slice_exp(ctx, &exp.span, exp_base, exp_index, exp_length)
        }
        el::ExpKind::Tuple(exps) => infer_tuple_exp(ctx, &exp.span, exps),
        el::ExpKind::Len(exp_inner) => infer_len_exp(ctx, &exp.span, exp_inner),
        el::ExpKind::Mem(exp_element, exp_set) => {
            infer_mem_exp(ctx, &exp.span, exp_element, exp_set)
        }
        el::ExpKind::Dot(exp_inner, atom) => infer_dot_exp(ctx, &exp.span, exp_inner, atom),
        el::ExpKind::Upd(exp_base, path, exp_field) => {
            infer_upd_exp(ctx, &exp.span, exp_base, path, exp_field)
        }
        el::ExpKind::Paren(exp_inner) => infer_paren_exp(ctx, &exp.span, exp_inner),
        el::ExpKind::Call(id, targs, args) => infer_call_exp(ctx, &exp.span, id, targs, args),
        el::ExpKind::Sub(exp_inner, plain_typ) => {
            infer_sub_exp(ctx, &exp.span, exp_inner, plain_typ)
        }
        el::ExpKind::Iter(exp_inner, iter) => infer_iter_exp(ctx, &exp.span, exp_inner, *iter),
        el::ExpKind::Eps => fail_infer(&exp.span, "empty sequence"),
        el::ExpKind::Str(_) => fail_infer(&exp.span, "struct expression"),
        el::ExpKind::Atom(_) => fail_infer(&exp.span, "atom"),
        el::ExpKind::Seq(_) => fail_infer(&exp.span, "sequence expression"),
        el::ExpKind::Infix(_, _, _) => fail_infer(&exp.span, "infix expression"),
        el::ExpKind::Brack(_, _, _) => fail_infer(&exp.span, "bracket expression"),
        el::ExpKind::Hole(_)
        | el::ExpKind::Fuse(_, _)
        | el::ExpKind::Unparen(_)
        | el::ExpKind::Latex(_) => fail_infer(&exp.span, "hint expression"),
    }
}

fn infer_exps(ctx: &mut Context, exps: &[el::Exp]) -> Attempt<Vec<il::Exp>> {
    let mut exps_il = Vec::with_capacity(exps.len());
    for exp in exps {
        let exp_il = infer_exp(ctx, exp)?;
        exps_il.push(exp_il);
    }
    Ok(exps_il)
}

// - Boolean expression inference

fn infer_bool_exp(_ctx: &mut Context, span: &Span, value: bool) -> Attempt<il::Exp> {
    let exp_il = note_phrase! {
        node: il::ExpKind::Bool(value),
        note: il::TypKind::Bool,
        span: span.clone(),
    };
    Ok(exp_il)
}

// - Number expression inference

fn infer_num_exp(_ctx: &mut Context, span: &Span, value: &el::Num) -> Attempt<il::Exp> {
    let exp_il = note_phrase! {
        node: il::ExpKind::Num(value.clone()),
        note: il::TypKind::Num(xl::num::to_typ(value)),
        span: span.clone(),
    };
    Ok(exp_il)
}

// - Text expression inference

fn infer_text_exp(_ctx: &mut Context, span: &Span, value: &el::Text) -> Attempt<il::Exp> {
    let exp_il = note_phrase! {
        node: il::ExpKind::Text(value.clone()),
        note: il::TypKind::Text,
        span: span.clone(),
    };
    Ok(exp_il)
}

// - Variable expression inference

fn infer_var_exp(ctx: &mut Context, span: &Span, id: &Id) -> Attempt<il::Exp> {
    let tid = xl::var::strip_var_suffix(id);
    let Some(typ_il) = ctx.find_metavar_opt(&tid) else {
        return fail_infer(&id.span, "variable");
    };
    let exp_il = note_phrase! {
        node: il::ExpKind::Var(id.clone()),
        note: typ_il.node.clone(),
        span: span.clone(),
    };
    Ok(exp_il)
}

// - Operator inference failures

fn operator_error<T>(span: Span) -> Attempt<T> {
    fail_attempt(
        ElabErrorKind::OperatorNotDefined,
        span,
        "operator is not defined for the operand types",
    )
}

// - Unary expression inference

fn infer_un_exp(ctx: &mut Context, span: &Span, op: el::UnOp, exp: &el::Exp) -> Attempt<il::Exp> {
    let exp_il = infer_exp(ctx, exp)?;
    let candidates_il = match op {
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
    for (op_typ_il, typ_il_operand, typ_il_result) in candidates_il {
        let typ_il_expect = typ_at(typ_il_operand, &exp_il.span);
        if let Ok(exp_il) = cast_exp(ctx, &typ_il_expect, exp_il.clone()) {
            let exp_il = note_phrase! {
                node: il::ExpKind::Un(op, op_typ_il, Box::new(exp_il)),
                note: typ_il_result,
                span: span.clone(),
            };
            return Ok(exp_il);
        }
    }
    operator_error(span.clone())
}

// - Binary expression inference

fn infer_bin_exp(
    ctx: &mut Context,
    span: &Span,
    exp_l: &el::Exp,
    op: el::BinOp,
    exp_r: &el::Exp,
) -> Attempt<il::Exp> {
    let exp_il_l = infer_exp(ctx, exp_l)?;
    let exp_il_r = infer_exp(ctx, exp_r)?;
    let candidates_il = match op {
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
    for (op_typ_il, typ_il_expect_l, typ_il_expect_r, typ_il_result) in candidates_il {
        let typ_il_expect_l = typ_at(typ_il_expect_l, &exp_il_l.span);
        let typ_il_expect_r = typ_at(typ_il_expect_r, &exp_il_r.span);
        let Ok(exp_il_l) = cast_exp(ctx, &typ_il_expect_l, exp_il_l.clone()) else {
            continue;
        };
        let Ok(exp_il_r) = cast_exp(ctx, &typ_il_expect_r, exp_il_r.clone()) else {
            continue;
        };
        let exp_il = note_phrase! {
            node: il::ExpKind::Bin(op, op_typ_il, Box::new(exp_il_l), Box::new(exp_il_r)),
            note: typ_il_result,
            span: span.clone(),
        };
        return Ok(exp_il);
    }
    operator_error(span.clone())
}

// - Comparison expression inference

fn infer_cmp_exp(
    ctx: &mut Context,
    span: &Span,
    exp_l: &el::Exp,
    op: el::CmpOp,
    exp_r: &el::Exp,
) -> Attempt<il::Exp> {
    match op {
        el::CmpOp::Bool(_) => choose_sequential(
            ctx,
            |ctx| {
                let exp_il_r = infer_exp(ctx, exp_r)?;
                let typ_il_l_expect =
                    phrase!(node: exp_il_r.note.as_ref().clone(), span: exp_il_r.span.clone());
                let exp_il_l = elab_exp(ctx, &typ_il_l_expect, exp_l)?;
                let exp_il = note_phrase! {
                    node: il::ExpKind::Cmp(op, il::OpTyp::Bool, Box::new(exp_il_l), Box::new(exp_il_r)),
                    note: il::TypKind::Bool,
                    span: span.clone(),
                };
                Ok(exp_il)
            },
            |ctx| {
                let exp_il_l = infer_exp(ctx, exp_l)?;
                let typ_il_r_expect =
                    phrase!(node: exp_il_l.note.as_ref().clone(), span: exp_il_l.span.clone());
                let exp_il_r = elab_exp(ctx, &typ_il_r_expect, exp_r)?;
                let exp_il = note_phrase! {
                    node: il::ExpKind::Cmp(op, il::OpTyp::Bool, Box::new(exp_il_l), Box::new(exp_il_r)),
                    note: il::TypKind::Bool,
                    span: span.clone(),
                };
                Ok(exp_il)
            },
        ),
        el::CmpOp::Num(_) => {
            let exp_il_l = infer_exp(ctx, exp_l)?;
            let exp_il_r = infer_exp(ctx, exp_r)?;
            for (op_typ_il, typ_il_expect_kind) in [
                (il::OpTyp::Nat, il::TypKind::Num(xl::num::Typ::Nat)),
                (il::OpTyp::Int, il::TypKind::Num(xl::num::Typ::Int)),
            ] {
                let typ_il_expect_l = typ_at(typ_il_expect_kind.clone(), &exp_il_l.span);
                let typ_il_expect_r = typ_at(typ_il_expect_kind, &exp_il_r.span);
                let Ok(exp_il_l) = cast_exp(ctx, &typ_il_expect_l, exp_il_l.clone()) else {
                    continue;
                };
                let Ok(exp_il_r) = cast_exp(ctx, &typ_il_expect_r, exp_il_r.clone()) else {
                    continue;
                };
                let exp_il = note_phrase! {
                    node: il::ExpKind::Cmp(op, op_typ_il, Box::new(exp_il_l), Box::new(exp_il_r)),
                    note: il::TypKind::Bool,
                    span: span.clone(),
                };
                return Ok(exp_il);
            }
            operator_error(span.clone())
        }
    }
}

// - Arithmetic expression inference

fn infer_arith_exp(ctx: &mut Context, span: &Span, exp: &el::Exp) -> Attempt<il::Exp> {
    let mut exp_il = infer_exp(ctx, exp)?;
    exp_il.span = span.clone();
    Ok(exp_il)
}

// - List expression inference

fn infer_list_exp(ctx: &mut Context, span: &Span, exps: &[el::Exp]) -> Attempt<il::Exp> {
    let Some((exp_first, exps_rest)) = exps.split_first() else {
        return fail_infer(span, "empty list");
    };
    let exp_il_first = infer_exp(ctx, exp_first)?;
    let typ_il_first =
        phrase!(node: exp_il_first.note.as_ref().clone(), span: exp_il_first.span.clone());
    let mut exps_il_rest = infer_exps(ctx, exps_rest)?;
    for exp_il in &exps_il_rest {
        let typ_il = phrase!(node: exp_il.note.as_ref().clone(), span: exp_il.span.clone());
        let equivalent = equiv_typ(&ctx.tdenv, &typ_il_first, &typ_il)?;
        if !equivalent {
            return fail_infer(span, "list with heterogeneous elements");
        }
    }
    let mut exps_il = vec![exp_il_first];
    exps_il.append(&mut exps_il_rest);
    let exp_il = note_phrase! {
        node: il::ExpKind::List(exps_il),
        note: il::TypKind::Iter(Box::new(typ_il_first), il::Iter::List),
        span: span.clone(),
    };
    Ok(exp_il)
}

// - Cons expression inference

fn infer_cons_exp(
    ctx: &mut Context,
    span: &Span,
    exp_head: &el::Exp,
    exp_tail: &el::Exp,
) -> Attempt<il::Exp> {
    let exp_il_head = infer_exp(ctx, exp_head)?;
    let typ_il_head =
        phrase!(node: exp_il_head.note.as_ref().clone(), span: exp_il_head.span.clone());
    let typ_il_list_kind = il::TypKind::Iter(Box::new(typ_il_head), il::Iter::List);
    let typ_il_list = phrase!(node: typ_il_list_kind, span: exp_il_head.span.clone());
    let exp_il_tail = elab_exp(ctx, &typ_il_list, exp_tail)?;
    let exp_il = note_phrase! {
        node: il::ExpKind::Cons(Box::new(exp_il_head), Box::new(exp_il_tail)),
        note: typ_il_list.node,
        span: span.clone(),
    };
    Ok(exp_il)
}

// - Concatenation expression inference

fn infer_cat_exp(
    ctx: &mut Context,
    span: &Span,
    exp_l: &el::Exp,
    exp_r: &el::Exp,
) -> Attempt<il::Exp> {
    choose_sequential(
        ctx,
        |ctx| {
            let exp_il_l = infer_exp(ctx, exp_l)?;
            let typ_il_l =
                phrase!(node: exp_il_l.note.as_ref().clone(), span: exp_il_l.span.clone());
            let typ_il_base = as_list_typ(ctx, &typ_il_l)?;
            let typ_il_list_kind = il::TypKind::Iter(Box::new(typ_il_base.clone()), il::Iter::List);
            let typ_il_list = phrase!(node: typ_il_list_kind, span: typ_il_base.span);
            let exp_il_r = elab_exp(ctx, &typ_il_list, exp_r)?;
            let exp_il = note_phrase! {
                node: il::ExpKind::Cat(Box::new(exp_il_l), Box::new(exp_il_r)),
                note: typ_il_list.node,
                span: span.clone(),
            };
            Ok(exp_il)
        },
        |ctx| {
            let typ_il_text_l = typ_at(il::TypKind::Text, &exp_l.span);
            let exp_il_l = elab_exp(ctx, &typ_il_text_l, exp_l)?;
            let typ_il_text_r = typ_at(il::TypKind::Text, &exp_r.span);
            let exp_il_r = elab_exp(ctx, &typ_il_text_r, exp_r)?;
            let exp_il = note_phrase! {
                node: il::ExpKind::Cat(Box::new(exp_il_l), Box::new(exp_il_r)),
                note: il::TypKind::Text,
                span: span.clone(),
            };
            Ok(exp_il)
        },
    )
}

// - Tuple expression inference

fn infer_tuple_exp(ctx: &mut Context, span: &Span, exps: &[el::Exp]) -> Attempt<il::Exp> {
    let exps_il = infer_exps(ctx, exps)?;
    let typs_il = exps_il
        .iter()
        .map(|exp_il| phrase!(node: exp_il.note.as_ref().clone(), span: exp_il.span.clone()))
        .collect();
    let exp_il = note_phrase! {
        node: il::ExpKind::Tuple(exps_il),
        note: il::TypKind::Tuple(typs_il),
        span: span.clone(),
    };
    Ok(exp_il)
}

// - Length expression inference

fn infer_len_exp(ctx: &mut Context, span: &Span, exp: &el::Exp) -> Attempt<il::Exp> {
    choose_sequential(
        ctx,
        |ctx| {
            let exp_il = infer_exp(ctx, exp)?;
            let typ_il = phrase!(node: exp_il.note.as_ref().clone(), span: exp_il.span.clone());
            as_list_typ(ctx, &typ_il)?;
            let exp_il = note_phrase! {
                node: il::ExpKind::Len(Box::new(exp_il)),
                note: il::TypKind::Num(xl::num::Typ::Nat),
                span: span.clone(),
            };
            Ok(exp_il)
        },
        |ctx| {
            let typ_il_text = typ_at(il::TypKind::Text, &exp.span);
            let exp_il = elab_exp(ctx, &typ_il_text, exp)?;
            let exp_il = note_phrase! {
                node: il::ExpKind::Len(Box::new(exp_il)),
                note: il::TypKind::Num(xl::num::Typ::Nat),
                span: span.clone(),
            };
            Ok(exp_il)
        },
    )
}

// - Membership expression inference

fn infer_mem_exp(
    ctx: &mut Context,
    span: &Span,
    exp_element: &el::Exp,
    exp_set: &el::Exp,
) -> Attempt<il::Exp> {
    choose_sequential(
        ctx,
        |ctx| {
            let exp_il_element = infer_exp(ctx, exp_element)?;
            let typ_il_element = phrase! {
                node: exp_il_element.note.as_ref().clone(),
                span: exp_il_element.span.clone(),
            };
            let typ_il_list_kind = il::TypKind::Iter(Box::new(typ_il_element), il::Iter::List);
            let typ_il_list = phrase!(node: typ_il_list_kind, span: exp_set.span.clone());
            let exp_il_set = elab_exp(ctx, &typ_il_list, exp_set)?;
            let exp_il = note_phrase! {
                node: il::ExpKind::Mem(Box::new(exp_il_element), Box::new(exp_il_set)),
                note: il::TypKind::Bool,
                span: span.clone(),
            };
            Ok(exp_il)
        },
        |ctx| {
            let exp_il_set = infer_exp(ctx, exp_set)?;
            let typ_il_set =
                phrase!(node: exp_il_set.note.as_ref().clone(), span: exp_il_set.span.clone());
            let typ_il_element = as_list_typ(ctx, &typ_il_set)?;
            let exp_il_element = elab_exp(ctx, &typ_il_element, exp_element)?;
            let exp_il = note_phrase! {
                node: il::ExpKind::Mem(Box::new(exp_il_element), Box::new(exp_il_set)),
                note: il::TypKind::Bool,
                span: span.clone(),
            };
            Ok(exp_il)
        },
    )
}

// - Index expression inference

fn infer_idx_exp(
    ctx: &mut Context,
    span: &Span,
    exp_base: &el::Exp,
    exp_index: &el::Exp,
) -> Attempt<il::Exp> {
    choose_sequential(
        ctx,
        |ctx| {
            let exp_il_base = infer_exp(ctx, exp_base)?;
            let typ_il_base =
                phrase!(node: exp_il_base.note.as_ref().clone(), span: exp_il_base.span.clone());
            let typ_il_element = as_list_typ(ctx, &typ_il_base)?;
            let typ_il_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_index.span);
            let exp_il_index = elab_exp(ctx, &typ_il_nat, exp_index)?;
            let exp_il = note_phrase! {
                node: il::ExpKind::Idx(Box::new(exp_il_base), Box::new(exp_il_index)),
                note: typ_il_element.node,
                span: span.clone(),
            };
            Ok(exp_il)
        },
        |ctx| {
            let typ_il_text = typ_at(il::TypKind::Text, &exp_base.span);
            let exp_il_base = elab_exp(ctx, &typ_il_text, exp_base)?;
            let typ_il_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_index.span);
            let exp_il_index = elab_exp(ctx, &typ_il_nat, exp_index)?;
            let exp_il = note_phrase! {
                node: il::ExpKind::Idx(Box::new(exp_il_base), Box::new(exp_il_index)),
                note: il::TypKind::Text,
                span: span.clone(),
            };
            Ok(exp_il)
        },
    )
}

// - Slice expression inference

fn infer_slice_exp(
    ctx: &mut Context,
    span: &Span,
    exp_base: &el::Exp,
    exp_index: &el::Exp,
    exp_length: &el::Exp,
) -> Attempt<il::Exp> {
    choose_sequential(
        ctx,
        |ctx| {
            let exp_il_base = infer_exp(ctx, exp_base)?;
            let typ_il_base =
                phrase!(node: exp_il_base.note.as_ref().clone(), span: exp_il_base.span.clone());
            as_list_typ(ctx, &typ_il_base)?;
            let typ_il_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_index.span);
            let exp_il_index = elab_exp(ctx, &typ_il_nat, exp_index)?;
            let typ_il_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_length.span);
            let exp_il_length = elab_exp(ctx, &typ_il_nat, exp_length)?;
            let exp_il = note_phrase! { node: il::ExpKind::Slice(
                Box::new(exp_il_base),
                Box::new(exp_il_index),
                Box::new(exp_il_length),
            ), note: typ_il_base.node, span: span.clone() };
            Ok(exp_il)
        },
        |ctx| {
            let typ_il_text = typ_at(il::TypKind::Text, &exp_base.span);
            let exp_il_base = elab_exp(ctx, &typ_il_text, exp_base)?;
            let typ_il_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_index.span);
            let exp_il_index = elab_exp(ctx, &typ_il_nat, exp_index)?;
            let typ_il_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_length.span);
            let exp_il_length = elab_exp(ctx, &typ_il_nat, exp_length)?;
            let exp_il = note_phrase! { node: il::ExpKind::Slice(
                Box::new(exp_il_base),
                Box::new(exp_il_index),
                Box::new(exp_il_length),
            ), note: il::TypKind::Text, span: span.clone() };
            Ok(exp_il)
        },
    )
}

// - Dot expression inference

fn infer_dot_exp(
    ctx: &mut Context,
    span: &Span,
    exp: &el::Exp,
    atom: &el::Atom,
) -> Attempt<il::Exp> {
    let exp_il = infer_exp(ctx, exp)?;
    let typ_il = phrase!(node: exp_il.note.as_ref().clone(), span: exp_il.span.clone());
    let typ_fields_il = as_struct_typ(ctx, &typ_il)?;
    let Some((_, typ_il_field)) = typ_fields_il
        .iter()
        .find(|(atom_field, _)| atom_field.node == atom.node)
    else {
        return fail_infer(&atom.span, "field");
    };
    let exp_il = note_phrase! {
        node: il::ExpKind::Dot(Box::new(exp_il), atom.clone()),
        note: typ_il_field.node.clone(),
        span: span.clone(),
    };
    Ok(exp_il)
}

// - Update expression inference

fn infer_upd_exp(
    ctx: &mut Context,
    span: &Span,
    exp_base: &el::Exp,
    path: &el::Path,
    exp_field: &el::Exp,
) -> Attempt<il::Exp> {
    let exp_il_base = infer_exp(ctx, exp_base)?;
    let typ_il_base =
        phrase!(node: exp_il_base.note.as_ref().clone(), span: exp_il_base.span.clone());
    let path_il = elab_path(ctx, &typ_il_base, path)?;
    let typ_il_field = typ_at(path_il.note.as_ref().clone(), &path_il.span);
    let exp_il_field = elab_exp(ctx, &typ_il_field, exp_field)?;
    let exp_il = note_phrase! {
        node: il::ExpKind::Upd(Box::new(exp_il_base), Box::new(path_il), Box::new(exp_il_field)),
        note: typ_il_base.node,
        span: span.clone(),
    };
    Ok(exp_il)
}

// - Parenthesized expression inference

fn infer_paren_exp(ctx: &mut Context, span: &Span, exp: &el::Exp) -> Attempt<il::Exp> {
    let mut exp_il = infer_exp(ctx, exp)?;
    exp_il.span = span.clone();
    Ok(exp_il)
}

// - Call expression inference

fn infer_call_exp(
    ctx: &mut Context,
    span: &Span,
    id: &Id,
    targs: &[el::Targ],
    args: &[el::Arg],
) -> Attempt<il::Exp> {
    let (tparams_il, params_il, typ_il_ret) = match ctx.find_func_signature(id) {
        Ok((tparams, params, typ_ret)) => (tparams.to_vec(), params.to_vec(), typ_ret.clone()),
        Err(error) => return fail(error),
    };
    if tparams_il.len() != targs.len() {
        return fail(arity_error(tparams_il.len(), targs.len(), id.span.clone()));
    }
    let mut targs_il = Vec::with_capacity(targs.len());
    for targ in targs {
        let targ_il = match elab_plain_typ(ctx, targ) {
            Ok(targ_il) => targ_il,
            Err(error) => return fail(error),
        };
        targs_il.push(targ_il);
    }
    let theta = match Theta::from_lists(&tparams_il, &targs_il) {
        Ok(theta) => theta,
        Err(mismatch) => {
            return fail(arity_error(
                mismatch.expected,
                mismatch.actual,
                id.span.clone(),
            ));
        }
    };
    let params_il = subst_params(&theta, &params_il)?;
    let typ_il_ret = subst_typ(&theta, &typ_il_ret)?;
    let args_il = elab_args(ctx, &params_il, args, false, span)?;
    let exp_il = note_phrase! {
        node: il::ExpKind::Call(id.clone(), targs_il, args_il),
        note: typ_il_ret.node,
        span: span.clone(),
    };
    Ok(exp_il)
}

// - Iterated expression inference

fn infer_iter_exp(
    ctx: &mut Context,
    span: &Span,
    exp: &el::Exp,
    iter: el::Iter,
) -> Attempt<il::Exp> {
    let exp_il = infer_exp(ctx, exp)?;
    let typ_il = phrase!(node: exp_il.note.as_ref().clone(), span: exp_il.span.clone());
    let iter_il = elab_iter(iter);
    let exp_il = note_phrase! {
        node: il::ExpKind::Iter(Box::new(exp_il), (iter_il, vec![])),
        note: il::TypKind::Iter(Box::new(typ_il), iter_il),
        span: span.clone(),
    };
    Ok(exp_il)
}

// - Subtype expression inference

fn infer_sub_exp(
    ctx: &mut Context,
    span: &Span,
    exp: &el::Exp,
    plain_typ: &el::PlainTyp,
) -> Attempt<il::Exp> {
    let exp_il = infer_exp(ctx, exp)?;
    let typ_il_source = phrase!(node: exp_il.note.as_ref().clone(), span: exp_il.span.clone());
    let typ_il_target = match elab_plain_typ(ctx, plain_typ) {
        Ok(typ_il) => typ_il,
        Err(error) => return fail(error),
    };
    let source_sub = sub_typ(&ctx.tdenv, &typ_il_source, &typ_il_target)?;
    let target_sub = sub_typ(&ctx.tdenv, &typ_il_target, &typ_il_source)?;
    if !source_sub && !target_sub {
        return fail_attempt(
            ElabErrorKind::TypeMismatch,
            exp_il.span.clone(),
            "subtype expression compares incomparable types",
        );
    }
    let check = optimize_sub_typ(&ctx.tdenv, &typ_il_source, &typ_il_target)?;
    let exp_il = note_phrase! {
        node: il::ExpKind::Sub(Box::new(exp_il), Box::new(typ_il_target), Box::new(check)),
        note: il::TypKind::Bool,
        span: span.clone(),
    };
    Ok(exp_il)
}

// - Expected-type expression elaboration

fn cast_exp(ctx: &Context, typ_il_expect: &il::Typ, exp_il: il::Exp) -> Attempt<il::Exp> {
    let typ_il_infer = phrase!(node: exp_il.note.as_ref().clone(), span: exp_il.span.clone());
    let equivalent = equiv_typ(&ctx.tdenv, typ_il_expect, &typ_il_infer)?;
    if equivalent {
        return Ok(exp_il);
    }
    let subtype = sub_typ(&ctx.tdenv, &typ_il_infer, typ_il_expect)?;
    if subtype {
        let span = exp_il.span.clone();
        let exp_il = note_phrase! {
            node: il::ExpKind::UpCast(Box::new(typ_il_expect.clone()), Box::new(exp_il)),
            note: typ_il_expect.node.clone(),
            span: span,
        };
        return Ok(exp_il);
    }
    fail_attempt(
        ElabErrorKind::InvalidCast,
        exp_il.span,
        "cannot cast inferred expression to expected type",
    )
}

fn respan_parenthesized_exp(exp_il: &mut il::Exp, span: &Span) {
    exp_il.span = span.clone();
    match &mut exp_il.node {
        il::ExpKind::UpCast(_, exp_il_inner) | il::ExpKind::DownCast(_, exp_il_inner) => {
            respan_parenthesized_exp(exp_il_inner, span);
        }
        _ => {}
    }
}

fn elab_exp(ctx: &mut Context, typ_il_expect: &il::Typ, exp: &el::Exp) -> Attempt<il::Exp> {
    let error = ElabError::new(
        ElabErrorKind::NoMatchingAlternative,
        exp.span.clone(),
        "expression elaboration failed",
    );
    let parenthesized = matches!(exp.node, el::ExpKind::Paren(_));
    let span = exp.span.clone();
    elab_exp_inner(ctx, typ_il_expect, exp)
        .map(move |mut exp_il| {
            if parenthesized {
                respan_parenthesized_exp(&mut exp_il, &span);
            }
            exp_il
        })
        .map_err(|failure| failure.nest(error))
}

fn elab_exp_inner(ctx: &mut Context, typ_il_expect: &il::Typ, exp: &el::Exp) -> Attempt<il::Exp> {
    if let Ok((typ_il_base, iter_il_expect)) = as_iter_typ(ctx, typ_il_expect) {
        return choose_sequential(
            ctx,
            |ctx| elab_singleton_iter_exp(ctx, typ_il_expect, &typ_il_base, iter_il_expect, exp),
            |ctx| elab_exp_normal(ctx, typ_il_expect, exp),
        );
    }
    elab_exp_normal(ctx, typ_il_expect, exp)
}

// - Singleton iteration expression elaboration

fn elab_singleton_iter_exp(
    ctx: &mut Context,
    typ_il_expect: &il::Typ,
    typ_il_base: &il::Typ,
    iter_il_expect: il::Iter,
    exp: &el::Exp,
) -> Attempt<il::Exp> {
    if matches!(&exp.node, el::ExpKind::Var(id) if id.node == "_")
        || matches!(&exp.node, el::ExpKind::Eps)
        || matches!(&exp.node, el::ExpKind::List(exps) if exps.is_empty())
    {
        return fail_silent();
    }
    let exp_il_inner = elab_exp(ctx, typ_il_base, exp)?;
    let exp_il_kind = match iter_il_expect {
        il::Iter::Opt => il::ExpKind::Opt(Some(Box::new(exp_il_inner))),
        il::Iter::List => il::ExpKind::List(vec![exp_il_inner]),
    };
    Ok(note_phrase! {
        node: exp_il_kind,
        note: typ_il_expect.node.clone(),
        span: exp.span.clone(),
    })
}

// - Normal expression elaboration

fn elab_exp_normal(ctx: &mut Context, typ_il_expect: &il::Typ, exp: &el::Exp) -> Attempt<il::Exp> {
    let checkpoint = ctx.checkpoint();
    match infer_exp(ctx, exp) {
        Ok(exp_il) => match cast_exp(ctx, typ_il_expect, exp_il) {
            Ok(exp_il) => {
                ctx.commit(checkpoint);
                Ok(exp_il)
            }
            Err(failure) => {
                ctx.rollback(checkpoint);
                Err(failure)
            }
        },
        Err(_) => {
            ctx.rollback(checkpoint);
            if matches!(&exp.node, el::ExpKind::Var(id) if id.node == "_") {
                return elab_wildcard_exp(ctx, typ_il_expect, exp);
            }
            if let il::TypKind::Var(id, targs_il) = &typ_il_expect.node
                && let Some(TypeDef::Defined(tparams, def_typ_il)) = ctx.find_typdef_opt(id)
            {
                let theta = match Theta::from_lists(tparams, targs_il) {
                    Ok(theta) => theta,
                    Err(mismatch) => {
                        return fail(arity_error(
                            mismatch.expected,
                            mismatch.actual,
                            typ_il_expect.span.clone(),
                        ));
                    }
                };
                match &def_typ_il.node {
                    il::DefTypKind::Plain(typ_il) => {
                        let typ_il = subst_typ(&theta, typ_il)?;
                        return elab_exp_normal(ctx, &typ_il, exp);
                    }
                    il::DefTypKind::Struct(typ_fields_il) => {
                        let mut typ_fields_il_subst = Vec::with_capacity(typ_fields_il.len());
                        for (atom, typ_il) in typ_fields_il {
                            let typ_il = subst_typ(&theta, typ_il)?;
                            typ_fields_il_subst.push((atom.clone(), typ_il));
                        }
                        return elab_struct_exp(ctx, typ_il_expect, &typ_fields_il_subst, exp);
                    }
                    il::DefTypKind::Variant(typ_cases_il) => {
                        let mut typ_cases_il_subst = Vec::with_capacity(typ_cases_il.len());
                        for (not_typ_il, origin_il, hints) in typ_cases_il {
                            let not_typ_il = subst_not_typ(&theta, not_typ_il)?;
                            let targs_il = subst_typs(&theta, &origin_il.node.1)?;
                            let origin_il = phrase! {
                                node: (origin_il.node.0.clone(), targs_il),
                                span: origin_il.span.clone(),
                            };
                            typ_cases_il_subst.push((not_typ_il, origin_il, hints.clone()));
                        }
                        return elab_variant_exp(ctx, typ_il_expect, &typ_cases_il_subst, exp);
                    }
                }
            }
            elab_plain_exp(ctx, typ_il_expect, exp)
        }
    }
}

// - Wildcard expression elaboration

fn elab_wildcard_exp(
    ctx: &mut Context,
    typ_il_expect: &il::Typ,
    exp: &el::Exp,
) -> Attempt<il::Exp> {
    let var_il =
        il_fresh::var_from_typ_wildcard(&ctx.menv, &ctx.frees, exp.span.clone(), typ_il_expect);
    let exp_il = il_var::as_exp(false, &var_il);
    ctx.add_free(var_il.id);
    Ok(exp_il)
}

// - Plain expression elaboration

fn elab_plain_exp(ctx: &mut Context, typ_il_expect: &il::Typ, exp: &el::Exp) -> Attempt<il::Exp> {
    let exp_il_kind = match &exp.node {
        el::ExpKind::Eps => elab_eps_exp(ctx, typ_il_expect)?,
        el::ExpKind::List(exps) => elab_list_exp(ctx, typ_il_expect, exps)?,
        el::ExpKind::Cons(exp_head, exp_tail) => {
            elab_cons_exp(ctx, typ_il_expect, exp_head, exp_tail)?
        }
        el::ExpKind::Cat(exp_l, exp_r) => elab_cat_exp(ctx, typ_il_expect, exp_l, exp_r)?,
        el::ExpKind::Tuple(exps) => elab_tuple_exp(ctx, typ_il_expect, exps)?,
        el::ExpKind::Paren(exp_inner) => elab_paren_exp(ctx, typ_il_expect, exp_inner)?,
        el::ExpKind::Iter(exp_inner, iter) => elab_iter_exp(ctx, typ_il_expect, exp_inner, *iter)?,
        _ => {
            return fail_attempt(
                ElabErrorKind::NoMatchingAlternative,
                exp.span.clone(),
                "expression requires unsupported contextual elaboration",
            );
        }
    };
    Ok(note_phrase! {
        node: exp_il_kind,
        note: typ_il_expect.node.clone(),
        span: exp.span.clone(),
    })
}

// - Epsilon expression elaboration

fn elab_eps_exp(ctx: &Context, typ_il_expect: &il::Typ) -> Attempt<il::ExpKind> {
    let (_, iter_il_expect) = as_iter_typ(ctx, typ_il_expect)?;
    Ok(match iter_il_expect {
        il::Iter::Opt => il::ExpKind::Opt(None),
        il::Iter::List => il::ExpKind::List(vec![]),
    })
}

// - List expression elaboration

fn elab_list_exp(
    ctx: &mut Context,
    typ_il_expect: &il::Typ,
    exps: &[el::Exp],
) -> Attempt<il::ExpKind> {
    let (typ_il_base, iter_il_expect) = as_iter_typ(ctx, typ_il_expect)?;
    if iter_il_expect != il::Iter::List {
        return fail_attempt(
            ElabErrorKind::InvalidIteration,
            typ_il_expect.span.clone(),
            "list expression has optional expected type",
        );
    }
    let mut exps_il = Vec::with_capacity(exps.len());
    for exp in exps {
        exps_il.push(elab_exp(ctx, &typ_il_base, exp)?);
    }
    Ok(il::ExpKind::List(exps_il))
}

// - Cons expression elaboration

fn elab_cons_exp(
    ctx: &mut Context,
    typ_il_expect: &il::Typ,
    exp_head: &el::Exp,
    exp_tail: &el::Exp,
) -> Attempt<il::ExpKind> {
    let (typ_il_base, iter_il_expect) = as_iter_typ(ctx, typ_il_expect)?;
    let exp_il_head = elab_exp(ctx, &typ_il_base, exp_head)?;
    let typ_il_tail_kind = il::TypKind::Iter(Box::new(typ_il_base), iter_il_expect);
    let typ_il_tail = phrase!(node: typ_il_tail_kind, span: typ_il_expect.span.clone());
    let exp_il_tail = elab_exp(ctx, &typ_il_tail, exp_tail)?;
    Ok(il::ExpKind::Cons(
        Box::new(exp_il_head),
        Box::new(exp_il_tail),
    ))
}

// - Concatenation expression elaboration

fn elab_cat_exp(
    ctx: &mut Context,
    typ_il_expect: &il::Typ,
    exp_l: &el::Exp,
    exp_r: &el::Exp,
) -> Attempt<il::ExpKind> {
    choose_sequential(
        ctx,
        |ctx| {
            let (typ_il_base, iter_il_expect) = as_iter_typ(ctx, typ_il_expect)?;
            let typ_il_iter_kind = il::TypKind::Iter(Box::new(typ_il_base.clone()), iter_il_expect);
            let typ_il_iter = phrase!(node: typ_il_iter_kind, span: typ_il_base.span);
            let exp_il_l = elab_exp(ctx, &typ_il_iter, exp_l)?;
            let exp_il_r = elab_exp(ctx, &typ_il_iter, exp_r)?;
            Ok(il::ExpKind::Cat(Box::new(exp_il_l), Box::new(exp_il_r)))
        },
        |ctx| {
            let typ_il_text = typ_at(il::TypKind::Text, &exp_l.span);
            let exp_il_l = elab_exp(ctx, &typ_il_text, exp_l)?;
            let typ_il_text = typ_at(il::TypKind::Text, &exp_r.span);
            let exp_il_r = elab_exp(ctx, &typ_il_text, exp_r)?;
            Ok(il::ExpKind::Cat(Box::new(exp_il_l), Box::new(exp_il_r)))
        },
    )
}

// - Tuple expression elaboration

fn elab_tuple_exp(
    ctx: &mut Context,
    typ_il_expect: &il::Typ,
    exps: &[el::Exp],
) -> Attempt<il::ExpKind> {
    let typs_il_expect = as_tuple_typ(ctx, typ_il_expect)?;
    if typs_il_expect.len() != exps.len() {
        return fail_attempt(
            ElabErrorKind::ArityMismatch,
            typ_il_expect.span.clone(),
            "tuple expression arity does not match",
        );
    }
    let mut exps_il = Vec::with_capacity(exps.len());
    for (typ_il_expect, exp) in typs_il_expect.iter().zip(exps) {
        exps_il.push(elab_exp(ctx, typ_il_expect, exp)?);
    }
    Ok(il::ExpKind::Tuple(exps_il))
}

// - Parenthesized expression elaboration

fn elab_paren_exp(
    ctx: &mut Context,
    typ_il_expect: &il::Typ,
    exp: &el::Exp,
) -> Attempt<il::ExpKind> {
    let exp_il = elab_exp(ctx, typ_il_expect, exp)?;
    Ok(exp_il.node)
}

// - Iterated expression elaboration

fn elab_iter_exp(
    ctx: &mut Context,
    typ_il_expect: &il::Typ,
    exp: &el::Exp,
    iter: el::Iter,
) -> Attempt<il::ExpKind> {
    let (typ_il_base, iter_il_expect) = as_iter_typ(ctx, typ_il_expect)?;
    let iter_il = elab_iter(iter);
    if iter_il != iter_il_expect {
        return fail_attempt(
            ElabErrorKind::InvalidIteration,
            exp.span.clone(),
            "iteration mismatch",
        );
    }
    let exp_il = elab_exp(ctx, &typ_il_base, exp)?;
    Ok(il::ExpKind::Iter(Box::new(exp_il), (iter_il, vec![])))
}

// - Notation expression elaboration

fn elab_not_exp(ctx: &mut Context, not_typ_il: &il::NotTyp, exp: &el::Exp) -> Attempt<il::NotExp> {
    if let el::ExpKind::Paren(exp) = &exp.node {
        return elab_not_exp(ctx, not_typ_il, exp);
    }
    match (&not_typ_il.node, &exp.node) {
        (Mixfix::Arg(typ_il), _) => {
            let exp_il = elab_exp(ctx, typ_il, exp)?;
            Ok(Mixfix::Arg(exp_il))
        }
        (Mixfix::Atom(atom_expect), el::ExpKind::Atom(atom)) if atom_expect.node == atom.node => {
            Ok(Mixfix::Atom(atom_expect.clone()))
        }
        (Mixfix::Seq(not_typs_il), el::ExpKind::Seq(exps)) => {
            if not_typs_il.len() != exps.len() {
                return fail_attempt(
                    ElabErrorKind::NoMatchingAlternative,
                    exp.span.clone(),
                    "notation sequence arity does not match",
                );
            }
            let mut not_exps_il = Vec::with_capacity(exps.len());
            for (not_typ_il_inner, exp) in not_typs_il.iter().zip(exps) {
                let not_typ_il_inner =
                    phrase!(node: not_typ_il_inner.clone(), span: not_typ_il.span.clone());
                let not_exp_il = elab_not_exp(ctx, &not_typ_il_inner, exp)?;
                not_exps_il.push(not_exp_il);
            }
            Ok(Mixfix::Seq(not_exps_il))
        }
        (
            Mixfix::Infix(not_typ_il_l, atom_expect, not_typ_il_r),
            el::ExpKind::Infix(exp_l, atom, exp_r),
        ) if atom_expect.node == atom.node => {
            let not_typ_il_l =
                phrase!(node: (**not_typ_il_l).clone(), span: not_typ_il.span.clone());
            let not_typ_il_r =
                phrase!(node: (**not_typ_il_r).clone(), span: not_typ_il.span.clone());
            let not_exp_il_l = elab_not_exp(ctx, &not_typ_il_l, exp_l)?;
            let not_exp_il_r = elab_not_exp(ctx, &not_typ_il_r, exp_r)?;
            Ok(Mixfix::Infix(
                Box::new(not_exp_il_l),
                atom_expect.clone(),
                Box::new(not_exp_il_r),
            ))
        }
        (
            Mixfix::Brack(atom_expect_l, not_typ_il_inner, atom_expect_r),
            el::ExpKind::Brack(atom_l, exp_inner, atom_r),
        ) if atom_expect_l.node == atom_l.node && atom_expect_r.node == atom_r.node => {
            let not_typ_il_inner = phrase! {
                node: (**not_typ_il_inner).clone(),
                span: not_typ_il.span.clone(),
            };
            let not_exp_il_inner = elab_not_exp(ctx, &not_typ_il_inner, exp_inner)?;
            Ok(Mixfix::Brack(
                atom_expect_l.clone(),
                Box::new(not_exp_il_inner),
                atom_expect_r.clone(),
            ))
        }
        _ => fail_attempt(
            ElabErrorKind::NoMatchingAlternative,
            exp.span.clone(),
            "expression does not match notation",
        ),
    }
}

// - Struct expression elaboration

fn elab_struct_exp(
    ctx: &mut Context,
    typ_il_expect: &il::Typ,
    typ_fields_il: &[il::TypField],
    exp: &el::Exp,
) -> Attempt<il::Exp> {
    let el::ExpKind::Str(exp_fields) = &exp.node else {
        return fail_attempt(
            ElabErrorKind::NoMatchingAlternative,
            exp.span.clone(),
            "expression is not a struct",
        );
    };
    if typ_fields_il.len() != exp_fields.len() {
        return fail_attempt(
            ElabErrorKind::ArityMismatch,
            exp.span.clone(),
            "struct field count does not match",
        );
    }
    let mut exp_fields_il = Vec::with_capacity(exp_fields.len());
    for ((atom_expect, typ_il), (atom, exp_field)) in typ_fields_il.iter().zip(exp_fields) {
        if atom_expect.node != atom.node {
            return fail_attempt(
                ElabErrorKind::TypeMismatch,
                atom.span.clone(),
                "struct field does not match",
            );
        }
        let exp_field_il = elab_exp(ctx, typ_il, exp_field)?;
        exp_fields_il.push((atom_expect.clone(), exp_field_il));
    }
    Ok(note_phrase! {
        node: il::ExpKind::Str(exp_fields_il),
        note: typ_il_expect.node.clone(),
        span: exp.span.clone(),
    })
}

// - Variant expression elaboration

fn elab_variant_exp(
    ctx: &mut Context,
    typ_il_expect: &il::Typ,
    typ_cases_il: &[il::TypCase],
    exp: &el::Exp,
) -> Attempt<il::Exp> {
    let checkpoint = ctx.checkpoint();
    let mut exps_il_match = Vec::new();
    for (not_typ_il, origin_il, _) in typ_cases_il {
        let candidate = ctx.checkpoint();
        let not_exp_il = match elab_not_exp(ctx, not_typ_il, exp) {
            Ok(not_exp_il) => not_exp_il,
            Err(_) => {
                ctx.rollback(candidate);
                continue;
            }
        };
        let typ_il_case_kind = il::TypKind::Var(origin_il.node.0.clone(), origin_il.node.1.clone());
        let typ_il_case = phrase!(node: typ_il_case_kind, span: origin_il.span.clone());
        let exp_il_case_kind = il::ExpKind::Case(Box::new(not_exp_il));
        let exp_il_case = note_phrase! {
            node: exp_il_case_kind,
            note: typ_il_case.node.clone(),
            span: exp.span.clone(),
        };
        let exp_il_case = match cast_exp(ctx, typ_il_expect, exp_il_case) {
            Ok(exp_il_case) => exp_il_case,
            Err(_) => {
                ctx.rollback(candidate);
                continue;
            }
        };
        ctx.commit(candidate);
        exps_il_match.push(exp_il_case);
    }
    match exps_il_match.len() {
        1 => {
            ctx.commit(checkpoint);
            Ok(exps_il_match.pop().expect("single variant match"))
        }
        0 => {
            ctx.rollback(checkpoint);
            fail_attempt(
                ElabErrorKind::NoMatchingAlternative,
                exp.span.clone(),
                "expression does not match any variant case",
            )
        }
        _ => {
            ctx.rollback(checkpoint);
            fail_attempt(
                ElabErrorKind::AmbiguousVariant,
                exp.span.clone(),
                "expression matches multiple variant cases",
            )
        }
    }
}

// == Paths

// - Path elaboration

fn elab_path(ctx: &mut Context, typ_il_expect: &il::Typ, path: &el::Path) -> Attempt<il::Path> {
    match &path.node {
        el::PathKind::Root => Ok(elab_root_path(&path.span, typ_il_expect)),
        el::PathKind::Idx(path_inner, exp_index) => {
            elab_idx_path(ctx, &path.span, typ_il_expect, path_inner, exp_index)
        }
        el::PathKind::Slice(path_inner, exp_index, exp_length) => elab_slice_path(
            ctx,
            &path.span,
            typ_il_expect,
            path_inner,
            exp_index,
            exp_length,
        ),
        el::PathKind::Dot(path_inner, atom) => {
            elab_dot_path(ctx, &path.span, typ_il_expect, path_inner, atom)
        }
    }
}

// - Root path elaboration

fn elab_root_path(span: &Span, typ_il_expect: &il::Typ) -> il::Path {
    note_phrase! {
        node: il::PathKind::Root,
        note: typ_il_expect.node.clone(),
        span: span.clone(),
    }
}

// - Index path elaboration

fn elab_idx_path(
    ctx: &mut Context,
    span: &Span,
    typ_il_expect: &il::Typ,
    path_inner: &el::Path,
    exp_index: &el::Exp,
) -> Attempt<il::Path> {
    choose_sequential(
        ctx,
        |ctx| {
            let path_il_inner = elab_path(ctx, typ_il_expect, path_inner)?;
            let typ_il_inner = typ_at(path_il_inner.note.as_ref().clone(), &path_il_inner.span);
            let typ_il_element = as_list_typ(ctx, &typ_il_inner)?;
            let typ_il_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_index.span);
            let exp_il_index = elab_exp(ctx, &typ_il_nat, exp_index)?;
            let path_il_kind = il::PathKind::Idx(Box::new(path_il_inner), Box::new(exp_il_index));
            Ok(note_phrase! {
                node: path_il_kind,
                note: typ_il_element.node,
                span: span.clone(),
            })
        },
        |ctx| {
            let path_il_inner = elab_path(ctx, typ_il_expect, path_inner)?;
            let typ_il_inner = typ_at(path_il_inner.note.as_ref().clone(), &path_il_inner.span);
            as_text_typ(ctx, &typ_il_inner)?;
            let typ_il_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_index.span);
            let exp_il_index = elab_exp(ctx, &typ_il_nat, exp_index)?;
            let path_il_kind = il::PathKind::Idx(Box::new(path_il_inner), Box::new(exp_il_index));
            Ok(note_phrase! {
                node: path_il_kind,
                note: typ_il_inner.node,
                span: span.clone(),
            })
        },
    )
}

// - Slice path elaboration

fn elab_slice_path(
    ctx: &mut Context,
    span: &Span,
    typ_il_expect: &il::Typ,
    path_inner: &el::Path,
    exp_index: &el::Exp,
    exp_length: &el::Exp,
) -> Attempt<il::Path> {
    let path_il_inner = elab_path(ctx, typ_il_expect, path_inner)?;
    let typ_il_inner = typ_at(path_il_inner.note.as_ref().clone(), &path_il_inner.span);
    let is_list = as_list_typ(ctx, &typ_il_inner).is_ok();
    let is_text = as_text_typ(ctx, &typ_il_inner).is_ok();
    if !is_list && !is_text {
        return fail_attempt(
            ElabErrorKind::CannotDestructure(TypeShape::List),
            typ_il_inner.span,
            "slice path requires a list or text",
        );
    }
    let typ_il_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_index.span);
    let exp_il_index = elab_exp(ctx, &typ_il_nat, exp_index)?;
    let typ_il_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_length.span);
    let exp_il_length = elab_exp(ctx, &typ_il_nat, exp_length)?;
    let path_il_kind = il::PathKind::Slice(
        Box::new(path_il_inner),
        Box::new(exp_il_index),
        Box::new(exp_il_length),
    );
    Ok(note_phrase! {
        node: path_il_kind,
        note: typ_il_inner.node,
        span: span.clone(),
    })
}

// - Dot path elaboration

fn elab_dot_path(
    ctx: &mut Context,
    span: &Span,
    typ_il_expect: &il::Typ,
    path_inner: &el::Path,
    atom: &el::Atom,
) -> Attempt<il::Path> {
    let path_il_inner = elab_path(ctx, typ_il_expect, path_inner)?;
    let typ_il_inner = typ_at(path_il_inner.note.as_ref().clone(), &path_il_inner.span);
    let typ_fields_il = as_struct_typ(ctx, &typ_il_inner)?;
    let Some((_, typ_il_field)) = typ_fields_il
        .into_iter()
        .find(|(atom_field, _)| atom_field.node == atom.node)
    else {
        return fail_infer(&atom.span, "field");
    };
    let path_il_kind = il::PathKind::Dot(Box::new(path_il_inner), atom.clone());
    Ok(note_phrase! {
        node: path_il_kind,
        note: typ_il_field.node,
        span: span.clone(),
    })
}

// == Parameters and arguments

// - Parameter elaboration

fn elab_param(ctx: &mut Context, param: &el::Param) -> Result<il::Param, ElabError> {
    let param_il_kind = match &param.node {
        el::ParamKind::Exp(plain_typ) => {
            let typ_il = elab_plain_typ(ctx, plain_typ)?;
            il::ParamKind::Exp(typ_il)
        }
        el::ParamKind::Def(id, tparams, params, plain_typ_ret) => {
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
            let (params_il, typ_il_ret) = {
                let mut ctx_local = ctx.scope();
                ctx_local.add_tparams(tparams)?;
                let params_il = params
                    .iter()
                    .map(|param| elab_param(&mut ctx_local, param))
                    .collect::<Result<Vec<_>, _>>()?;
                let typ_il_ret = elab_plain_typ(&ctx_local, plain_typ_ret)?;
                (params_il, typ_il_ret)
            };
            il::ParamKind::Def(id.clone(), tparams.clone(), params_il, typ_il_ret)
        }
    };
    let param_il = phrase!(node: param_il_kind, span: param.span.clone());
    Ok(param_il)
}

fn typ_of_param(param_il: &il::Param) -> il::Typ {
    match &param_il.node {
        il::ParamKind::Exp(typ_il) => typ_il.clone(),
        il::ParamKind::Def(_, tparams_il, params_il, typ_il_ret) => {
            let func_typ_il = il::FuncTyp {
                tparams: tparams_il.clone(),
                typs_params: params_il.iter().map(typ_of_param).collect(),
                typ_ret: Box::new(typ_il_ret.clone()),
            };
            let typ_il_kind = il::TypKind::Func(func_typ_il);
            phrase!(node: typ_il_kind, span: param_il.span.clone())
        }
    }
}

// - Argument elaboration

fn elab_arg(
    ctx: &mut Context,
    param_il: &il::Param,
    arg: &el::Arg,
    as_def: bool,
) -> Attempt<il::Arg> {
    match (&param_il.node, &arg.node) {
        (il::ParamKind::Exp(typ_il), el::ArgKind::Exp(exp)) => {
            let exp_il = elab_exp(ctx, typ_il, exp)?;
            let arg_il = il::ArgKind::Exp(Box::new(exp_il));
            let arg_il = phrase!(node: arg_il, span: arg.span.clone());
            Ok(arg_il)
        }
        (
            il::ParamKind::Def(id_param, tparams_il, params_il, typ_il_ret),
            el::ArgKind::Def(id_arg),
        ) if as_def => {
            if id_param.node != id_arg.node {
                return fail_attempt(
                    ElabErrorKind::InvalidArgument,
                    arg.span.clone(),
                    "function argument does not match its declared parameter",
                );
            }
            if let Err(error) = ctx.add_defined_func(
                id_param.clone(),
                tparams_il.clone(),
                params_il.clone(),
                typ_il_ret.clone(),
            ) {
                return fail(error);
            }
            let arg_il = il::ArgKind::Def(id_arg.clone());
            let arg_il = phrase!(node: arg_il, span: arg.span.clone());
            Ok(arg_il)
        }
        (il::ParamKind::Def(_, tparams_il, params_il, typ_il_ret), el::ArgKind::Def(id_arg)) => {
            let (tparams_il_arg, params_il_arg, typ_il_ret_arg) =
                match ctx.find_func_signature(id_arg) {
                    Ok(signature) => signature,
                    Err(error) => return fail(error),
                };
            let typ_il_param = il::FuncTyp {
                tparams: tparams_il.clone(),
                typs_params: params_il.iter().map(typ_of_param).collect(),
                typ_ret: Box::new(typ_il_ret.clone()),
            };
            let typ_il_arg = il::FuncTyp {
                tparams: tparams_il_arg.to_vec(),
                typs_params: params_il_arg.iter().map(typ_of_param).collect(),
                typ_ret: Box::new(typ_il_ret_arg.clone()),
            };
            let equivalent = equiv_func_typ(&ctx.tdenv, &arg.span, &typ_il_param, &typ_il_arg)?;
            if !equivalent {
                return fail_attempt(
                    ElabErrorKind::InvalidArgument,
                    arg.span.clone(),
                    "function argument type does not match",
                );
            }
            let arg_il = il::ArgKind::Def(id_arg.clone());
            let arg_il = phrase!(node: arg_il, span: arg.span.clone());
            Ok(arg_il)
        }
        _ => fail_attempt(
            ElabErrorKind::InvalidArgument,
            arg.span.clone(),
            "argument kind does not match parameter kind",
        ),
    }
}

fn elab_args(
    ctx: &mut Context,
    params_il: &[il::Param],
    args: &[el::Arg],
    as_def: bool,
    span: &Span,
) -> Attempt<Vec<il::Arg>> {
    if params_il.len() != args.len() {
        return fail_attempt(
            ElabErrorKind::ArityMismatch,
            span.clone(),
            "argument count does not match parameter count",
        );
    }
    let mut args_il = Vec::with_capacity(args.len());
    for (param_il, arg) in params_il.iter().zip(args) {
        let arg_il = elab_arg(ctx, param_il, arg, as_def)?;
        args_il.push(arg_il);
    }
    Ok(args_il)
}

// == Premises

#[allow(clippy::large_enum_variant)]
enum PremInternal {
    Some(il::Prem),
    Var,
    Else,
}

// - Premise elaboration

fn elab_prem(ctx: &mut Context, prem: &el::Prem) -> Attempt<PremInternal> {
    let prem_il_kind = match &prem.node {
        el::PremKind::Var(var_prem) => {
            elab_var_prem(ctx, var_prem)?;
            return Ok(PremInternal::Var);
        }
        el::PremKind::Rule(rule_prem) => elab_rule_prem(ctx, rule_prem)?,
        el::PremKind::RuleNot(rule_not_prem) => elab_rule_not_prem(ctx, rule_not_prem)?,
        el::PremKind::If(if_prem) => elab_if_prem(ctx, if_prem)?,
        el::PremKind::Else => return Ok(PremInternal::Else),
        el::PremKind::Iter(iter_prem) => elab_iter_prem(ctx, iter_prem)?,
        el::PremKind::Debug(debug_prem) => elab_debug_prem(ctx, debug_prem)?,
    };
    let prem_il = phrase!(node: prem_il_kind, span: prem.span.clone());
    Ok(PremInternal::Some(prem_il))
}

fn elab_prems(
    ctx: &mut Context,
    prems: &[el::Prem],
    span: &Span,
) -> Attempt<(Vec<il::Prem>, bool)> {
    let mut prems_il = Vec::new();
    let mut else_count = 0;
    for prem in prems {
        let prem_internal = elab_prem(ctx, prem)?;
        match prem_internal {
            PremInternal::Some(prem_il) => prems_il.push(prem_il),
            PremInternal::Var => {}
            PremInternal::Else => else_count += 1,
        }
    }
    if else_count > 1 {
        return fail_attempt(
            ElabErrorKind::InvalidPremise,
            span.clone(),
            "cannot use multiple otherwise premises",
        );
    }
    Ok((prems_il, else_count == 1))
}

// - Variable premise elaboration

fn elab_var_prem(ctx: &mut Context, prem: &el::VarPrem) -> Attempt<()> {
    if !valid_tid(&prem.id) {
        return fail_attempt(
            ElabErrorKind::InvalidIdentifier,
            prem.id.span.clone(),
            "invalid meta-variable identifier",
        );
    }
    if ctx.bound_typdef(&prem.id) {
        return fail_attempt(
            ElabErrorKind::Duplicate(EntityKind::Type),
            prem.id.span.clone(),
            "type already defined",
        );
    }
    let typ_il = match elab_plain_typ(ctx, &prem.plain_typ) {
        Ok(typ_il) => typ_il,
        Err(error) => return fail(error),
    };
    if let Err(error) = ctx.add_metavar(prem.id.clone(), typ_il) {
        return fail(error);
    }
    Ok(())
}

// - Rule premise elaboration

fn elab_rule_prem(ctx: &mut Context, prem: &el::RulePrem) -> Attempt<il::PremKind> {
    let (not_typ_il, input_hint) = match ctx.find_rel_signature(&prem.id) {
        Ok((not_typ_il, input_hint)) => (not_typ_il.clone(), input_hint.clone()),
        Err(error) => return fail(error),
    };
    let not_exp_il = elab_not_exp(ctx, &not_typ_il, &prem.exp)?;
    let exps_il = not_exp_il.args();
    let conditional = match input::is_conditional(&input_hint, &exps_il) {
        Ok(conditional) => conditional,
        Err(error) => {
            return fail_attempt(
                ElabErrorKind::InvalidInputHint,
                prem.exp.span.clone(),
                error.to_string(),
            );
        }
    };
    if conditional {
        Ok(il::PremKind::IfHold(il::IfHoldPrem {
            id: prem.id.clone(),
            not_exp: not_exp_il,
        }))
    } else {
        Ok(il::PremKind::Rule(il::RulePrem {
            id: prem.id.clone(),
            not_exp: not_exp_il,
            input_hint,
        }))
    }
}

// - Negated rule premise elaboration

fn elab_rule_not_prem(ctx: &mut Context, prem: &el::RuleNotPrem) -> Attempt<il::PremKind> {
    let (not_typ_il, input_hint) = match ctx.find_rel_signature(&prem.id) {
        Ok((not_typ_il, input_hint)) => (not_typ_il.clone(), input_hint.clone()),
        Err(error) => return fail(error),
    };
    let not_exp_il = elab_not_exp(ctx, &not_typ_il, &prem.exp)?;
    let exps_il = not_exp_il.args();
    let conditional = match input::is_conditional(&input_hint, &exps_il) {
        Ok(conditional) => conditional,
        Err(error) => {
            return fail_attempt(
                ElabErrorKind::InvalidInputHint,
                prem.exp.span.clone(),
                error.to_string(),
            );
        }
    };
    if !conditional {
        return fail_attempt(
            ElabErrorKind::InvalidPremise,
            prem.exp.span.clone(),
            "negated rule premise takes outputs",
        );
    }
    Ok(il::PremKind::IfNotHold(il::IfNotHoldPrem {
        id: prem.id.clone(),
        not_exp: not_exp_il,
    }))
}

// - Conditional premise elaboration

fn elab_if_prem(ctx: &mut Context, prem: &el::IfPrem) -> Attempt<il::PremKind> {
    let typ_il_bool = typ_at(il::TypKind::Bool, &prem.exp.span);
    let exp_il = elab_exp(ctx, &typ_il_bool, &prem.exp)?;
    Ok(il::PremKind::If(il::IfPrem { exp: exp_il }))
}

// - Iteration premise elaboration

fn elab_iter_prem(ctx: &mut Context, prem: &el::IterPrem) -> Attempt<il::PremKind> {
    let prem_il_inner = elab_prem(ctx, &prem.prem)?;
    let PremInternal::Some(prem_il_inner) = prem_il_inner else {
        return fail_attempt(
            ElabErrorKind::InvalidIteration,
            prem.prem.span.clone(),
            "cannot iterate variable or otherwise premise",
        );
    };
    let prem_iter_il = il::PremIter {
        iter: elab_iter(prem.iter),
        vars_bound: vec![],
        vars_bind: vec![],
    };
    Ok(il::PremKind::Iter(il::IterPrem {
        prem: Box::new(prem_il_inner),
        prem_iter: prem_iter_il,
    }))
}

// - Debug premise elaboration

fn elab_debug_prem(ctx: &mut Context, prem: &el::DebugPrem) -> Attempt<il::PremKind> {
    let exp_il = infer_exp(ctx, &prem.exp)?;
    Ok(il::PremKind::Debug(il::DebugPrem { exp: exp_il }))
}

// == Rules and clauses

// - Rule elaboration

fn elab_rule(
    ctx: &mut Context,
    rule: &el::Rule,
    id_rel: &Id,
    not_typ_il: &il::NotTyp,
) -> Result<(il::Rule, bool), ElabError> {
    let (id_rel_rule, id_rule, exp, prems) = &rule.node;
    if id_rel_rule.node != id_rel.node {
        return Err(ElabError::new(
            ElabErrorKind::InvalidRule,
            id_rule.span.clone(),
            "rule relation does not match its group",
        ));
    }
    let mut ctx_local = ctx.scope();
    ctx_local.reset_frees();
    let frees = rule.free();
    ctx_local.add_frees(&frees);
    let not_exp_il = finish(elab_not_exp(&mut ctx_local, not_typ_il, exp))?;
    let (prems_il, is_else) = finish(elab_prems(&mut ctx_local, prems, &id_rule.span))?;
    let rule_il_kind = il::RuleKind {
        id: id_rule.clone(),
        not_exp: not_exp_il,
        prems: prems_il,
    };
    let rule_il = phrase!(node: rule_il_kind, span: rule.span.clone());
    Ok((rule_il, is_else))
}

fn elab_rule_group(
    ctx: &mut Context,
    def: &Phrase<&el::RuleGroupDef>,
) -> Result<(Option<il::RuleGroup>, Option<il::ElseGroup>), ElabError> {
    let span = &def.span;
    let def = def.node;
    let Rel::Defined {
        not_typ: not_typ_il,
        ..
    } = ctx.find_defined_rel(&def.relid)?
    else {
        unreachable!("checked defined relation")
    };
    let not_typ_il = not_typ_il.clone();
    let mut rules_il = Vec::with_capacity(def.rules.len());
    let mut rules_il_else = Vec::new();
    for rule in &def.rules {
        let (rule_il, is_else) = elab_rule(ctx, rule, &def.relid, &not_typ_il)?;
        if is_else {
            rules_il_else.push(rule_il);
        } else {
            rules_il.push(rule_il);
        }
    }
    match rules_il_else.len() {
        0 => {
            let rule_group_il = (def.groupid.clone(), rules_il);
            let rule_group_il = phrase!(node: rule_group_il, span: span.clone());
            Ok((Some(rule_group_il), None))
        }
        1 if def.rules.len() == 1 => {
            let else_group_il = (def.groupid.clone(), rules_il_else.remove(0));
            let else_group_il = phrase!(node: else_group_il, span: span.clone());
            Ok((None, Some(else_group_il)))
        }
        _ => Err(ElabError::new(
            ElabErrorKind::InvalidRule,
            span.clone(),
            "invalid otherwise rule group",
        )),
    }
}

// - Clause elaboration

fn elab_clause(
    ctx: &mut Context,
    def: &Phrase<&el::FuncDef>,
) -> Result<(il::Clause, bool), ElabError> {
    let span = &def.span;
    let def = def.node;
    let Func::Defined {
        tparams: tparams_il_expect,
        params: params_il,
        typ_ret: typ_il_ret,
        ..
    } = ctx.find_defined_func(&def.id)?
    else {
        unreachable!("checked defined function")
    };
    if def.tparams.len() != tparams_il_expect.len()
        || def
            .tparams
            .iter()
            .zip(tparams_il_expect)
            .any(|(tparam, tparam_il_expect)| tparam.node != tparam_il_expect.node)
    {
        return Err(ElabError::new(
            ElabErrorKind::ArityMismatch,
            def.id.span.clone(),
            "type parameters do not match",
        ));
    }
    let params_il = params_il.to_vec();
    let typ_il_ret = typ_il_ret.clone();
    let mut ctx_local = ctx.scope();
    ctx_local.reset_frees();
    let frees = def.free();
    ctx_local.add_frees(&frees);
    ctx_local.add_tparams(&def.tparams)?;
    let args_il = finish(elab_args(&mut ctx_local, &params_il, &def.args, true, span))?;
    let (prems_il, is_else) = finish(elab_prems(&mut ctx_local, &def.prems, span))?;
    let exp_il = finish(elab_exp(&mut ctx_local, &typ_il_ret, &def.exp))?;
    let clause_il_kind = il::ClauseKind {
        args: args_il,
        expression: exp_il,
        premises: prems_il,
    };
    let clause_il = phrase!(node: clause_il_kind, span: span.clone());
    Ok((clause_il, is_else))
}

// == Definitions

// - Definition dispatch

fn elab_def(ctx: &mut Context, def_el: el::Def) -> Result<Option<il::Def>, ElabError> {
    let span = def_el.span;
    match def_el.node {
        el::DefKind::ExternSyntax(extern_syntax_def) => {
            let def_il_kind = elab_extern_syntax_def(ctx, extern_syntax_def)?;
            let def_il = phrase!(node: def_il_kind, span: span);
            Ok(Some(def_il))
        }
        el::DefKind::Syntax(syntax_def) => {
            elab_syntax_def(ctx, &syntax_def)?;
            Ok(None)
        }
        el::DefKind::Typ(typ_def) => {
            let span = typ_def.def_typ.span.clone();
            let def_il_kind = elab_typ_def(ctx, typ_def)?;
            let def_il = phrase!(node: def_il_kind, span: span);
            Ok(Some(def_il))
        }
        el::DefKind::Var(var_def) => {
            let span = var_def.id.span.clone();
            let def_il_kind = elab_var_def(ctx, var_def)?;
            let def_il = phrase!(node: def_il_kind, span: span);
            Ok(Some(def_il))
        }
        el::DefKind::ExternRel(extern_rel_def) => {
            let def_il_kind = elab_extern_rel_def(ctx, extern_rel_def, &span)?;
            let def_il = phrase!(node: def_il_kind, span: span);
            Ok(Some(def_il))
        }
        el::DefKind::Rel(rel_def) => {
            let def_il_kind = elab_rel_def(ctx, rel_def, &span)?;
            let def_il = phrase!(node: def_il_kind, span: span);
            Ok(Some(def_il))
        }
        el::DefKind::RuleGroup(rule_group_def) => {
            let rule_group_def = crate::phrase! {
                node: &rule_group_def,
                span: span,
            };
            elab_rule_group_def(ctx, &rule_group_def)?;
            Ok(None)
        }
        el::DefKind::ExternDec(extern_dec_def) => {
            let def_il_kind = elab_extern_dec_def(ctx, extern_dec_def)?;
            let def_il = phrase!(node: def_il_kind, span: span);
            Ok(Some(def_il))
        }
        el::DefKind::BuiltinDec(builtin_dec_def) => {
            let def_il_kind = elab_builtin_dec_def(ctx, builtin_dec_def)?;
            let def_il = phrase!(node: def_il_kind, span: span);
            Ok(Some(def_il))
        }
        el::DefKind::TableDec(table_dec_def) => {
            let def_il_kind = elab_table_dec_def(ctx, table_dec_def, &span)?;
            let def_il = phrase!(node: def_il_kind, span: span);
            Ok(Some(def_il))
        }
        el::DefKind::FuncDec(func_dec_def) => {
            let def_il_kind = elab_func_dec_def(ctx, func_dec_def)?;
            let def_il = phrase!(node: def_il_kind, span: span);
            Ok(Some(def_il))
        }
        el::DefKind::TableDef(table_def) => {
            elab_table_def(ctx, &table_def)?;
            Ok(None)
        }
        el::DefKind::FuncDef(func_def) => {
            let func_def = crate::phrase! {
                node: &func_def,
                span: span,
            };
            elab_func_def(ctx, &func_def)?;
            Ok(None)
        }
        el::DefKind::Sep => Ok(None),
    }
}

// - Type declarations

fn elab_extern_syntax_def(
    ctx: &mut Context,
    def: el::ExternSyntaxDef,
) -> Result<il::DefKind, ElabError> {
    if !valid_tid(&def.id) {
        return Err(ElabError::new(
            ElabErrorKind::InvalidIdentifier,
            def.id.span.clone(),
            "invalid type identifier",
        ));
    }
    ctx.add_typdef(def.id.clone(), TypeDef::Extern)?;
    let typ_il_kind = il::TypKind::Var(def.id.clone(), vec![]);
    let typ_il = phrase!(node: typ_il_kind, span: def.id.span.clone());
    ctx.add_metavar(def.id.clone(), typ_il)?;
    let extern_typ_il = il::ExternTyp {
        id: def.id,
        hints: def.hints,
    };
    Ok(il::DefKind::ExternTyp(extern_typ_il))
}

fn elab_syntax_def(ctx: &mut Context, def: &el::SyntaxDef) -> Result<(), ElabError> {
    for entry in &def.entries {
        distinct_tparams(&entry.tparams, &entry.id.span)?;
        if !valid_tid(&entry.id) {
            return Err(ElabError::new(
                ElabErrorKind::InvalidIdentifier,
                entry.id.span.clone(),
                "invalid type identifier",
            ));
        }
        ctx.add_typdef(entry.id.clone(), TypeDef::Defining(entry.tparams.clone()))?;
        if entry.tparams.is_empty() {
            let typ_il_kind = il::TypKind::Var(entry.id.clone(), vec![]);
            let typ_il = phrase!(node: typ_il_kind, span: entry.id.span.clone());
            ctx.add_metavar(entry.id.clone(), typ_il)?;
        }
    }
    Ok(())
}

// - Type definitions

fn elab_typ_def(ctx: &mut Context, def: el::TypDef) -> Result<il::DefKind, ElabError> {
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
            ctx.add_typdef(def.id.clone(), TypeDef::Defining(def.tparams.clone()))?;
            if def.tparams.is_empty() {
                let typ_il_kind = il::TypKind::Var(def.id.clone(), vec![]);
                let typ_il = phrase!(node: typ_il_kind, span: def.id.span.clone());
                ctx.add_metavar(def.id.clone(), typ_il)?;
            }
        }
    }
    let (type_def, def_typ_il) = {
        let mut ctx_local = ctx.scope();
        ctx_local.add_tparams(&def.tparams)?;
        elab_def_typ(&ctx_local, &def.id, &def.tparams, &def.def_typ)?
    };
    ctx.update_typdef(&def.id, type_def)?;
    let typ_def_il = il::TypDef {
        id: def.id,
        tparams: def.tparams,
        def_typ: def_typ_il,
        hints: def.hints,
    };
    Ok(il::DefKind::Typ(typ_def_il))
}

// - Variable definitions

fn elab_var_def(ctx: &mut Context, def: el::VarDef) -> Result<il::DefKind, ElabError> {
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
    let typ_il = elab_plain_typ(ctx, &def.plain_typ)?;
    ctx.add_metavar(def.id.clone(), typ_il.clone())?;
    let var_def_il = il::VarDef {
        id: def.id,
        typ: typ_il,
        hints: def.hints,
    };
    Ok(il::DefKind::Var(var_def_il))
}

// - Input hints

fn fetch_input_hint(
    span: &Span,
    not_typ_il: &il::NotTyp,
    hints: &[el::Hint],
) -> Result<input::InputHint, ElabError> {
    let arity = not_typ_il.node.arity();
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

// - Relation definitions

fn elab_extern_rel_def(
    ctx: &mut Context,
    def: el::ExternRelDef,
    span: &Span,
) -> Result<il::DefKind, ElabError> {
    let typ = el::Typ::Notation(def.not_typ.clone());
    let not_typ_il = elab_not_typ(ctx, &typ)?;
    let input_hint = fetch_input_hint(span, &not_typ_il, &def.hints)?;
    ctx.add_extern_rel(def.id.clone(), not_typ_il.clone(), input_hint.clone())?;
    let rel_il = il::ExternRel {
        id: def.id,
        not_typ: not_typ_il,
        input_hint,
        hints: def.hints,
    };
    Ok(il::DefKind::ExternRel(rel_il))
}

fn elab_rel_def(ctx: &mut Context, def: el::RelDef, span: &Span) -> Result<il::DefKind, ElabError> {
    let typ = el::Typ::Notation(def.not_typ.clone());
    let not_typ_il = elab_not_typ(ctx, &typ)?;
    let input_hint = fetch_input_hint(span, &not_typ_il, &def.hints)?;
    ctx.add_defined_rel(def.id.clone(), not_typ_il.clone(), input_hint.clone())?;
    let rel_il = il::Rel {
        id: def.id,
        not_typ: not_typ_il,
        input_hint,
        rule_groups: vec![],
        else_group: None,
        hints: def.hints,
    };
    Ok(il::DefKind::Rel(rel_il))
}

// - Rule group definitions

fn elab_rule_group_def(
    ctx: &mut Context,
    def: &Phrase<&el::RuleGroupDef>,
) -> Result<(), ElabError> {
    let (rule_group_il, else_group_il) = elab_rule_group(ctx, def)?;
    let def = def.node;
    if let Some(rule_group_il) = rule_group_il {
        ctx.add_defined_rule_group(&def.relid, rule_group_il)?;
    }
    if let Some(else_group_il) = else_group_il {
        ctx.add_defined_else_group(&def.relid, else_group_il)?;
    }
    Ok(())
}

// - Function declarations

fn elab_extern_dec_def(ctx: &mut Context, def: el::ExternDecDef) -> Result<il::DefKind, ElabError> {
    distinct_tparams(&def.tparams, &def.id.span)?;
    let (params_il, typ_il) = {
        let mut ctx_local = ctx.scope();
        ctx_local.add_tparams(&def.tparams)?;
        let params_il = def
            .params
            .iter()
            .map(|param| elab_param(&mut ctx_local, param))
            .collect::<Result<Vec<_>, _>>()?;
        let typ_il = elab_plain_typ(&ctx_local, &def.plain_typ)?;
        (params_il, typ_il)
    };
    ctx.add_extern_func(
        def.id.clone(),
        def.tparams.clone(),
        params_il.clone(),
        typ_il.clone(),
    )?;
    let dec_il = il::ExternDec {
        id: def.id,
        tparams: def.tparams,
        params: params_il,
        typ: typ_il,
        hints: def.hints,
    };
    Ok(il::DefKind::ExternDec(dec_il))
}

fn elab_builtin_dec_def(
    ctx: &mut Context,
    def: el::BuiltinDecDef,
) -> Result<il::DefKind, ElabError> {
    distinct_tparams(&def.tparams, &def.id.span)?;
    let (params_il, typ_il) = {
        let mut ctx_local = ctx.scope();
        ctx_local.add_tparams(&def.tparams)?;
        let params_il = def
            .params
            .iter()
            .map(|param| elab_param(&mut ctx_local, param))
            .collect::<Result<Vec<_>, _>>()?;
        let typ_il = elab_plain_typ(&ctx_local, &def.plain_typ)?;
        (params_il, typ_il)
    };
    ctx.add_builtin_func(
        def.id.clone(),
        def.tparams.clone(),
        params_il.clone(),
        typ_il.clone(),
    )?;
    let dec_il = il::BuiltinDec {
        id: def.id,
        tparams: def.tparams,
        params: params_il,
        typ: typ_il,
        hints: def.hints,
    };
    Ok(il::DefKind::BuiltinDec(dec_il))
}

fn elab_table_dec_def(
    ctx: &mut Context,
    def: el::TableDecDef,
    span: &Span,
) -> Result<il::DefKind, ElabError> {
    let params_il = def
        .params
        .iter()
        .map(|param| elab_param(ctx, param))
        .collect::<Result<Vec<_>, _>>()?;
    if params_il
        .iter()
        .any(|param_il| !matches!(param_il.node, il::ParamKind::Exp(_)))
    {
        return Err(ElabError::new(
            ElabErrorKind::InvalidDefinition,
            span.clone(),
            "table cannot have function parameters",
        ));
    }
    let typ_il = elab_plain_typ(ctx, &def.plain_typ)?;
    if typ_il.node != il::TypKind::Bool {
        return Err(ElabError::new(
            ElabErrorKind::TypeMismatch,
            typ_il.span,
            "table must return boolean",
        ));
    }
    ctx.add_table_func(def.id.clone(), params_il.clone(), typ_il.clone())?;
    let table_dec_il = il::TableDec {
        id: def.id,
        params: params_il,
        typ: typ_il,
        rows: vec![],
        hints: def.hints,
    };
    Ok(il::DefKind::TableDec(table_dec_il))
}

fn elab_func_dec_def(ctx: &mut Context, def: el::FuncDecDef) -> Result<il::DefKind, ElabError> {
    distinct_tparams(&def.tparams, &def.id.span)?;
    let (params_il, typ_il) = {
        let mut ctx_local = ctx.scope();
        ctx_local.add_tparams(&def.tparams)?;
        let params_il = def
            .params
            .iter()
            .map(|param| elab_param(&mut ctx_local, param))
            .collect::<Result<Vec<_>, _>>()?;
        let typ_il = elab_plain_typ(&ctx_local, &def.plain_typ)?;
        (params_il, typ_il)
    };
    ctx.add_defined_func(
        def.id.clone(),
        def.tparams.clone(),
        params_il.clone(),
        typ_il.clone(),
    )?;
    let dec_il = il::FuncDec {
        id: def.id,
        tparams: def.tparams,
        params: params_il,
        typ: typ_il,
        clauses: vec![],
        else_clause: None,
        hints: def.hints,
    };
    Ok(il::DefKind::FuncDec(dec_il))
}

// - Table function definitions

fn elab_table_def(ctx: &mut Context, def: &el::TableDef) -> Result<(), ElabError> {
    let Func::Table {
        params: params_il,
        typ_ret: typ_il,
        ..
    } = ctx.find_table_func(&def.id)?
    else {
        unreachable!("checked table function")
    };
    let params_il = params_il.to_vec();
    let typ_il = typ_il.clone();
    let mut rows_il = Vec::with_capacity(def.rows.len());
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
                let arg = el::ArgKind::Exp(Box::new(exp));
                phrase!(node: arg, span: span)
            })
            .collect::<Vec<_>>();
        let (args_il, exp_il_body) = {
            let mut ctx_local = ctx.scope();
            ctx_local.reset_frees();
            let frees = row.free();
            ctx_local.add_frees(&frees);
            let args_il = finish(elab_args(
                &mut ctx_local,
                &params_il,
                &args,
                true,
                &row.span,
            ))?;
            let exp_il_body = finish(elab_exp(&mut ctx_local, &typ_il, exp_body))?;
            (args_il, exp_il_body)
        };
        let row_il = phrase!(node: (args_il, exp_il_body), span: row.span.clone());
        rows_il.push(row_il);
    }
    ctx.add_table_func_rows(&def.id, rows_il)?;
    Ok(())
}

// - Function definitions

fn elab_func_def(ctx: &mut Context, def: &Phrase<&el::FuncDef>) -> Result<(), ElabError> {
    let (clause_il, is_else) = elab_clause(ctx, def)?;
    let def = def.node;
    if is_else {
        ctx.add_defined_func_else_clause(&def.id, clause_il)?;
    } else {
        ctx.add_defined_func_clause(&def.id, clause_il)?;
    }
    Ok(())
}

// == Specification

// - Definition population

fn populate_defs(ctx: &Context, defs_il: il::Spec) -> Result<il::Spec, ElabError> {
    defs_il
        .into_iter()
        .map(|def_il| {
            let def_il_kind = match def_il.node {
                il::DefKind::Rel(mut rel_il) => {
                    if !rel_il.rule_groups.is_empty() || rel_il.else_group.is_some() {
                        return Err(ElabError::new(
                            ElabErrorKind::AlreadyPopulated,
                            def_il.span,
                            "relation was already populated",
                        ));
                    }
                    let Rel::Defined {
                        rule_groups: rule_groups_il,
                        else_group: else_group_il,
                        ..
                    } = ctx.find_defined_rel(&rel_il.id)?
                    else {
                        unreachable!("checked defined relation")
                    };
                    rel_il.rule_groups = rule_groups_il.to_vec();
                    rel_il.else_group = else_group_il.as_deref().cloned();
                    il::DefKind::Rel(rel_il)
                }
                il::DefKind::TableDec(mut table_il) => {
                    if !table_il.rows.is_empty() {
                        return Err(ElabError::new(
                            ElabErrorKind::AlreadyPopulated,
                            def_il.span,
                            "table was already populated",
                        ));
                    }
                    let Func::Table {
                        table_rows: rows_il,
                        ..
                    } = ctx.find_table_func(&table_il.id)?
                    else {
                        unreachable!("checked table function")
                    };
                    table_il.rows = rows_il.to_vec();
                    il::DefKind::TableDec(table_il)
                }
                il::DefKind::FuncDec(mut func_il) => {
                    if !func_il.clauses.is_empty() || func_il.else_clause.is_some() {
                        return Err(ElabError::new(
                            ElabErrorKind::AlreadyPopulated,
                            def_il.span,
                            "function was already populated",
                        ));
                    }
                    let Func::Defined {
                        clauses: clauses_il,
                        else_clause: else_clause_il,
                        ..
                    } = ctx.find_defined_func(&func_il.id)?
                    else {
                        unreachable!("checked defined function")
                    };
                    func_il.clauses = clauses_il.to_vec();
                    func_il.else_clause = else_clause_il.as_deref().cloned();
                    il::DefKind::FuncDec(func_il)
                }
                def_il_kind => def_il_kind,
            };
            let def_il = phrase!(node: def_il_kind, span: def_il.span);
            Ok(def_il)
        })
        .collect()
}

// - Entry point

pub(super) fn elaborate(spec_el: el::Spec) -> Result<il::Spec, ElabError> {
    let mut ctx = Context::new();
    let mut defs_il = Vec::new();
    for def_el in spec_el {
        if let Some(def_il) = elab_def(&mut ctx, def_el)? {
            defs_il.push(def_il);
        }
    }
    let mut defs_il = populate_defs(&ctx, defs_il)?;
    dimension::analyze_spec(&mut defs_il)?;
    Ok(defs_il)
}
