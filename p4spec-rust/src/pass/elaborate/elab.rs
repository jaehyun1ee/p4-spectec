//! Elaboration-language validation and conversion to intermediate syntax

use crate::{
    lang::{
        common::{
            Id, ds::map::ArityMismatch, notation::mixfix::Mixfix, noted::Noted, source::Span,
        },
        el::ast as el,
        hints::input,
        il::{ast as il, fresh as il_fresh, var as il_var},
        traits::free::Free,
        xl,
    },
    runtime::types::{
        Theta, TypeArityMismatch, TypeDef, TypeErrorKind, equiv_func_typ, equiv_typ, expand_typ,
        optimize_sub_typ, sub_typ, subst_not_typ, subst_params, subst_typ, subst_typs,
    },
    spanned,
};

use super::{
    ElabError, ElabErrorKind, EntityKind, TypeShape,
    attempt::{Attempt, choose_sequential, fail, fail_silent, finish},
    context::Context,
    dimension,
};

// == Iteration

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

fn as_text_typ(ctx: &Context, typ: &il::Typ) -> Attempt<()> {
    let typ = expand_typ(&ctx.tdenv, typ)?;
    match typ.node {
        il::TypKind::Text => Ok(()),
        _ => fail(destruct_error(TypeShape::Text, typ.span)),
    }
}

fn as_iter_typ(ctx: &Context, typ: &il::Typ) -> Attempt<(il::Typ, il::Iter)> {
    let typ = expand_typ(&ctx.tdenv, typ)?;
    match typ.node {
        il::TypKind::Iter(typ, iter) => Ok((*typ, iter)),
        _ => fail(destruct_error(TypeShape::Iteration, typ.span)),
    }
}

fn as_tuple_typ(ctx: &Context, typ: &il::Typ) -> Attempt<Vec<il::Typ>> {
    let typ = expand_typ(&ctx.tdenv, typ)?;
    match typ.node {
        il::TypKind::Tuple(typs) => Ok(typs),
        _ => fail(destruct_error(TypeShape::Tuple, typ.span)),
    }
}

fn as_list_typ(ctx: &Context, typ: &il::Typ) -> Attempt<il::Typ> {
    let typ = expand_typ(&ctx.tdenv, typ)?;
    match typ.node {
        il::TypKind::Iter(typ, il::Iter::List) => Ok(*typ),
        _ => fail(destruct_error(TypeShape::List, typ.span)),
    }
}

fn as_struct_typ(ctx: &Context, typ: &il::Typ) -> Attempt<Vec<il::TypField>> {
    let typ = expand_typ(&ctx.tdenv, typ)?;
    let il::TypKind::Var(id, _) = &typ.node else {
        return fail(destruct_error(TypeShape::Struct, typ.span));
    };
    let Some(TypeDef::Defined(_, def_typ)) = ctx.find_typdef_opt(id) else {
        return fail(destruct_error(TypeShape::Struct, typ.span));
    };
    match &def_typ.node {
        il::DefTypKind::Struct(fields) => Ok(fields.clone()),
        _ => fail(destruct_error(TypeShape::Struct, typ.span)),
    }
}

// - Plain and notation types

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
                let targ_il = elab_plain_typ(ctx, targ)?;
                targs_il.push(targ_il);
            }
            il::TypKind::Var(id.clone(), targs_il)
        }
        el::PlainTypKind::Paren(plain_typ) => {
            let typ = elab_plain_typ(ctx, plain_typ)?;
            typ.node
        }
        el::PlainTypKind::Tuple(plain_typs) => {
            let mut typs = Vec::with_capacity(plain_typs.len());
            for plain_typ in plain_typs {
                let typ = elab_plain_typ(ctx, plain_typ)?;
                typs.push(typ);
            }
            il::TypKind::Tuple(typs)
        }
        el::PlainTypKind::Iter(plain_typ, iter) => {
            let typ = elab_plain_typ(ctx, plain_typ)?;
            il::TypKind::Iter(Box::new(typ), elab_iter(*iter))
        }
    };
    let typ = spanned!(node: kind, span: plain_typ.span.clone());
    Ok(typ)
}

fn elab_not_typ(ctx: &Context, typ: &el::Typ) -> Result<il::NotTyp, ElabError> {
    match typ {
        el::Typ::Plain(plain_typ) => {
            let typ = elab_plain_typ(ctx, plain_typ)?;
            let not_typ = Mixfix::Arg(typ);
            let not_typ = spanned!(node: not_typ, span: plain_typ.span.clone());
            Ok(not_typ)
        }
        el::Typ::Notation(not_typ) => {
            let mixfix = match &not_typ.node {
                el::NotTypKind::Atom(atom) => Mixfix::Atom(atom.clone()),
                el::NotTypKind::Seq(typs) => {
                    let mut items = Vec::with_capacity(typs.len());
                    for typ in typs {
                        let typ = elab_not_typ(ctx, typ)?;
                        items.push(typ.node);
                    }
                    Mixfix::Seq(items)
                }
                el::NotTypKind::Infix(typ_l, atom, typ_r) => {
                    let typ_l = elab_not_typ(ctx, typ_l)?;
                    let typ_r = elab_not_typ(ctx, typ_r)?;
                    Mixfix::Infix(Box::new(typ_l.node), atom.clone(), Box::new(typ_r.node))
                }
                el::NotTypKind::Brack(atom_l, typ, atom_r) => {
                    let typ = elab_not_typ(ctx, typ)?;
                    Mixfix::Brack(atom_l.clone(), Box::new(typ.node), atom_r.clone())
                }
            };
            let not_typ = spanned!(node: mixfix, span: not_typ.span.clone());
            Ok(not_typ)
        }
    }
}

// - Definition types

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
                .map(|(not_typ, origin, hints)| {
                    let not_typ = subst_not_typ(&theta, not_typ).map_err(ElabError::from)?;
                    let targs = subst_typs(&theta, &origin.node.1).map_err(ElabError::from)?;
                    let origin = spanned! {
                        node: (origin.node.0.clone(), targs),
                        span: origin.span.clone(),
                    };
                    Ok((not_typ, origin, hints.clone()))
                })
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
            let def_typ = il::DefTypKind::Plain(typ);
            spanned!(node: def_typ, span: plain_typ.span.clone())
        }
        el::DefTypKind::Struct(fields) => {
            let mut fields_il = Vec::with_capacity(fields.len());
            for (atom, plain_typ, _) in fields {
                let typ = elab_plain_typ(ctx, plain_typ)?;
                fields_il.push((atom.clone(), typ));
            }
            let def_typ_kind = il::DefTypKind::Struct(fields_il);
            spanned!(node: def_typ_kind, span: def_typ.span.clone())
        }
        el::DefTypKind::Variant(cases) => {
            let targs = tparams
                .iter()
                .map(|tparam| {
                    let typ = il::TypKind::Var(tparam.clone(), vec![]);
                    spanned!(node: typ, span: tparam.span.clone())
                })
                .collect();
            let origin_node = (id.clone(), targs);
            let origin = spanned!(node: origin_node, span: id.span.clone());
            let mut cases_il = vec![];
            for (typ, hints) in cases {
                match typ {
                    el::Typ::Plain(plain_typ) => {
                        let typ = elab_plain_typ(ctx, plain_typ)?;
                        let cases = elab_typ_case_plain(ctx, &typ)?;
                        cases_il.extend(cases);
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
            let def_typ_kind = il::DefTypKind::Variant(cases_il);
            spanned!(node: def_typ_kind, span: def_typ.span.clone())
        }
    };
    let typdef = TypeDef::Defined(tparams.to_vec(), Box::new(def_typ_il.clone()));
    Ok((typdef, def_typ_il))
}

fn fail_attempt<T>(kind: ElabErrorKind, span: Span, message: impl Into<String>) -> Attempt<T> {
    fail(ElabError::new(kind, span, message))
}

fn fail_infer<T>(span: Span, construct: &str) -> Attempt<T> {
    fail_attempt(
        ElabErrorKind::CannotInfer,
        span,
        format!("cannot infer type of {construct}"),
    )
}

fn inferred_exp(kind: il::ExpKind, typ: il::TypKind, span: Span) -> (il::Exp, il::Typ) {
    let exp = Noted::new(kind, typ.clone());
    let exp = spanned!(node: exp, span: span.clone());
    let typ = spanned!(node: typ, span: span);
    (exp, typ)
}

fn typ_at(kind: il::TypKind, span: &Span) -> il::Typ {
    spanned!(node: kind, span: span.clone())
}

// == Expressions

// - Expression type inference

fn infer_exp(ctx: &mut Context, exp: &el::Exp) -> Attempt<(il::Exp, il::Typ)> {
    let span = exp.span.clone();
    let (kind, typ) = match &exp.node {
        el::ExpKind::Bool(value) => infer_bool_exp(ctx, *value)?,
        el::ExpKind::Num(_, value) => infer_num_exp(ctx, value)?,
        el::ExpKind::Text(value) => infer_text_exp(ctx, value)?,
        el::ExpKind::Var(id) => infer_var_exp(ctx, id)?,
        el::ExpKind::Un(op, exp_inner) => infer_un_exp(ctx, &span, *op, exp_inner)?,
        el::ExpKind::Bin(exp_l, op, exp_r) => infer_bin_exp(ctx, &span, exp_l, *op, exp_r)?,
        el::ExpKind::Cmp(exp_l, op, exp_r) => infer_cmp_exp(ctx, &span, exp_l, *op, exp_r)?,
        el::ExpKind::Arith(exp_inner) => infer_arith_exp(ctx, exp_inner)?,
        el::ExpKind::List(exps) => infer_list_exp(ctx, &span, exps)?,
        el::ExpKind::Cons(exp_head, exp_tail) => infer_cons_exp(ctx, exp_head, exp_tail)?,
        el::ExpKind::Cat(exp_l, exp_r) => infer_cat_exp(ctx, exp_l, exp_r)?,
        el::ExpKind::Idx(exp_base, exp_index) => infer_idx_exp(ctx, exp_base, exp_index)?,
        el::ExpKind::Slice(exp_base, exp_index, exp_length) => {
            infer_slice_exp(ctx, exp_base, exp_index, exp_length)?
        }
        el::ExpKind::Tuple(exps) => infer_tuple_exp(ctx, exps)?,
        el::ExpKind::Len(exp_inner) => infer_len_exp(ctx, exp_inner)?,
        el::ExpKind::Mem(exp_element, exp_set) => infer_mem_exp(ctx, exp_element, exp_set)?,
        el::ExpKind::Dot(exp_inner, atom) => infer_dot_exp(ctx, exp_inner, atom)?,
        el::ExpKind::Upd(exp_base, path, exp_field) => {
            infer_upd_exp(ctx, exp_base, path, exp_field)?
        }
        el::ExpKind::Paren(exp_inner) => infer_paren_exp(ctx, exp_inner)?,
        el::ExpKind::Call(id, targs, args) => infer_call_exp(ctx, &span, id, targs, args)?,
        el::ExpKind::Sub(exp_inner, plain_typ) => infer_sub_exp(ctx, exp_inner, plain_typ)?,
        el::ExpKind::Iter(exp_inner, iter) => infer_iter_exp(ctx, exp_inner, *iter)?,
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
    Ok((exp, typ))
}

// - Literal and variable expressions

fn infer_bool_exp(_ctx: &mut Context, value: bool) -> Attempt<(il::ExpKind, il::TypKind)> {
    Ok((il::ExpKind::Bool(value), il::TypKind::Bool))
}

fn infer_num_exp(_ctx: &mut Context, value: &el::Num) -> Attempt<(il::ExpKind, il::TypKind)> {
    Ok((
        il::ExpKind::Num(value.clone()),
        il::TypKind::Num(xl::num::to_typ(value)),
    ))
}

fn infer_text_exp(_ctx: &mut Context, value: &el::Text) -> Attempt<(il::ExpKind, il::TypKind)> {
    Ok((il::ExpKind::Text(value.clone()), il::TypKind::Text))
}

fn infer_var_exp(ctx: &mut Context, id: &Id) -> Attempt<(il::ExpKind, il::TypKind)> {
    let tid = xl::var::strip_var_suffix(id);
    let Some(typ) = ctx.find_metavar_opt(&tid) else {
        return fail_infer(id.span.clone(), "variable");
    };
    Ok((il::ExpKind::Var(id.clone()), typ.node.clone()))
}

// - Operator expressions

fn operator_error<T>(span: Span) -> Attempt<T> {
    fail_attempt(
        ElabErrorKind::OperatorNotDefined,
        span,
        "operator is not defined for the operand types",
    )
}

fn infer_un_exp(
    ctx: &mut Context,
    span: &Span,
    op: el::UnOp,
    exp: &el::Exp,
) -> Attempt<(il::ExpKind, il::TypKind)> {
    let (exp, typ) = infer_exp(ctx, exp)?;
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
        if let Ok(exp) = cast_exp(ctx, &expected, &typ, exp.clone()) {
            return Ok((il::ExpKind::Un(op, op_typ, Box::new(exp)), typ_result));
        }
    }
    operator_error(span.clone())
}

fn infer_bin_exp(
    ctx: &mut Context,
    span: &Span,
    exp_l: &el::Exp,
    op: el::BinOp,
    exp_r: &el::Exp,
) -> Attempt<(il::ExpKind, il::TypKind)> {
    let (exp_l, typ_l) = infer_exp(ctx, exp_l)?;
    let (exp_r, typ_r) = infer_exp(ctx, exp_r)?;
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
        let Ok(exp_l) = cast_exp(ctx, &expected_l, &typ_l, exp_l.clone()) else {
            continue;
        };
        let Ok(exp_r) = cast_exp(ctx, &expected_r, &typ_r, exp_r.clone()) else {
            continue;
        };
        return Ok((
            il::ExpKind::Bin(op, op_typ, Box::new(exp_l), Box::new(exp_r)),
            result,
        ));
    }
    operator_error(span.clone())
}

fn infer_cmp_exp(
    ctx: &mut Context,
    span: &Span,
    exp_l: &el::Exp,
    op: el::CmpOp,
    exp_r: &el::Exp,
) -> Attempt<(il::ExpKind, il::TypKind)> {
    match op {
        el::CmpOp::Bool(_) => choose_sequential(
            ctx,
            |ctx| {
                let (exp_r, typ_r) = infer_exp(ctx, exp_r)?;
                let exp_l = elab_exp(ctx, &typ_r, exp_l)?;
                Ok((
                    il::ExpKind::Cmp(op, il::OpTyp::Bool, Box::new(exp_l), Box::new(exp_r)),
                    il::TypKind::Bool,
                ))
            },
            |ctx| {
                let (exp_l, typ_l) = infer_exp(ctx, exp_l)?;
                let exp_r = elab_exp(ctx, &typ_l, exp_r)?;
                Ok((
                    il::ExpKind::Cmp(op, il::OpTyp::Bool, Box::new(exp_l), Box::new(exp_r)),
                    il::TypKind::Bool,
                ))
            },
        ),
        el::CmpOp::Num(_) => {
            let (exp_l, typ_l) = infer_exp(ctx, exp_l)?;
            let (exp_r, typ_r) = infer_exp(ctx, exp_r)?;
            for (op_typ, expected_kind) in [
                (il::OpTyp::Nat, il::TypKind::Num(xl::num::Typ::Nat)),
                (il::OpTyp::Int, il::TypKind::Num(xl::num::Typ::Int)),
            ] {
                let expected_l = typ_at(expected_kind.clone(), &typ_l.span);
                let expected_r = typ_at(expected_kind, &typ_r.span);
                let Ok(exp_l) = cast_exp(ctx, &expected_l, &typ_l, exp_l.clone()) else {
                    continue;
                };
                let Ok(exp_r) = cast_exp(ctx, &expected_r, &typ_r, exp_r.clone()) else {
                    continue;
                };
                return Ok((
                    il::ExpKind::Cmp(op, op_typ, Box::new(exp_l), Box::new(exp_r)),
                    il::TypKind::Bool,
                ));
            }
            operator_error(span.clone())
        }
    }
}

fn infer_exps(ctx: &mut Context, exps: &[el::Exp]) -> Attempt<(Vec<il::Exp>, Vec<il::Typ>)> {
    let mut exps_il = Vec::with_capacity(exps.len());
    let mut typs_il = Vec::with_capacity(exps.len());
    for exp in exps {
        let (exp_il, typ_il) = infer_exp(ctx, exp)?;
        exps_il.push(exp_il);
        typs_il.push(typ_il);
    }
    Ok((exps_il, typs_il))
}

// - Sequence expressions

fn infer_arith_exp(ctx: &mut Context, exp: &el::Exp) -> Attempt<(il::ExpKind, il::TypKind)> {
    let (exp, typ) = infer_exp(ctx, exp)?;
    Ok((exp.node.kind, typ.node))
}

fn infer_list_exp(
    ctx: &mut Context,
    span: &Span,
    exps: &[el::Exp],
) -> Attempt<(il::ExpKind, il::TypKind)> {
    let Some((exp_first, exps_rest)) = exps.split_first() else {
        return fail_infer(span.clone(), "empty list");
    };
    let (exp_first, typ_first) = infer_exp(ctx, exp_first)?;
    let (mut exps_rest, typs_rest) = infer_exps(ctx, exps_rest)?;
    for typ in &typs_rest {
        let equivalent = equiv_typ(&ctx.tdenv, &typ_first, typ)?;
        if !equivalent {
            return fail_infer(span.clone(), "list with heterogeneous elements");
        }
    }
    let mut exps = vec![exp_first];
    exps.append(&mut exps_rest);
    let typ = il::TypKind::Iter(Box::new(typ_first), il::Iter::List);
    Ok((il::ExpKind::List(exps), typ))
}

fn infer_cons_exp(
    ctx: &mut Context,
    exp_head: &el::Exp,
    exp_tail: &el::Exp,
) -> Attempt<(il::ExpKind, il::TypKind)> {
    let (exp_head, typ_head) = infer_exp(ctx, exp_head)?;
    let typ_list_kind = il::TypKind::Iter(Box::new(typ_head.clone()), il::Iter::List);
    let typ_list = spanned!(node: typ_list_kind, span: typ_head.span.clone());
    let exp_tail = elab_exp(ctx, &typ_list, exp_tail)?;
    Ok((
        il::ExpKind::Cons(Box::new(exp_head), Box::new(exp_tail)),
        typ_list.node,
    ))
}

fn infer_cat_exp(
    ctx: &mut Context,
    exp_l: &el::Exp,
    exp_r: &el::Exp,
) -> Attempt<(il::ExpKind, il::TypKind)> {
    choose_sequential(
        ctx,
        |ctx| {
            let (exp_l, typ_l) = infer_exp(ctx, exp_l)?;
            let typ_base = as_list_typ(ctx, &typ_l)?;
            let typ_list_kind = il::TypKind::Iter(Box::new(typ_base.clone()), il::Iter::List);
            let typ_list = spanned!(node: typ_list_kind, span: typ_base.span);
            let exp_r = elab_exp(ctx, &typ_list, exp_r)?;
            Ok((
                il::ExpKind::Cat(Box::new(exp_l), Box::new(exp_r)),
                typ_list.node,
            ))
        },
        |ctx| {
            let typ_text = typ_at(il::TypKind::Text, &exp_l.span);
            let exp_l = elab_exp(ctx, &typ_text, exp_l)?;
            let typ_text_r = typ_at(il::TypKind::Text, &exp_r.span);
            let exp_r = elab_exp(ctx, &typ_text_r, exp_r)?;
            Ok((
                il::ExpKind::Cat(Box::new(exp_l), Box::new(exp_r)),
                il::TypKind::Text,
            ))
        },
    )
}

fn infer_tuple_exp(ctx: &mut Context, exps: &[el::Exp]) -> Attempt<(il::ExpKind, il::TypKind)> {
    let (exps, typs) = infer_exps(ctx, exps)?;
    Ok((il::ExpKind::Tuple(exps), il::TypKind::Tuple(typs)))
}

fn infer_len_exp(ctx: &mut Context, exp: &el::Exp) -> Attempt<(il::ExpKind, il::TypKind)> {
    choose_sequential(
        ctx,
        |ctx| {
            let (exp, typ) = infer_exp(ctx, exp)?;
            as_list_typ(ctx, &typ)?;
            Ok((
                il::ExpKind::Len(Box::new(exp)),
                il::TypKind::Num(xl::num::Typ::Nat),
            ))
        },
        |ctx| {
            let typ_text = typ_at(il::TypKind::Text, &exp.span);
            let exp = elab_exp(ctx, &typ_text, exp)?;
            Ok((
                il::ExpKind::Len(Box::new(exp)),
                il::TypKind::Num(xl::num::Typ::Nat),
            ))
        },
    )
}

fn infer_mem_exp(
    ctx: &mut Context,
    exp_element: &el::Exp,
    exp_set: &el::Exp,
) -> Attempt<(il::ExpKind, il::TypKind)> {
    choose_sequential(
        ctx,
        |ctx| {
            let (exp_element, typ_element) = infer_exp(ctx, exp_element)?;
            let typ_list_kind = il::TypKind::Iter(Box::new(typ_element), il::Iter::List);
            let typ_list = spanned!(node: typ_list_kind, span: exp_set.span.clone());
            let exp_set = elab_exp(ctx, &typ_list, exp_set)?;
            Ok((
                il::ExpKind::Mem(Box::new(exp_element), Box::new(exp_set)),
                il::TypKind::Bool,
            ))
        },
        |ctx| {
            let (exp_set, typ_set) = infer_exp(ctx, exp_set)?;
            let typ_element = as_list_typ(ctx, &typ_set)?;
            let exp_element = elab_exp(ctx, &typ_element, exp_element)?;
            Ok((
                il::ExpKind::Mem(Box::new(exp_element), Box::new(exp_set)),
                il::TypKind::Bool,
            ))
        },
    )
}

// - Access and update expressions

fn infer_idx_exp(
    ctx: &mut Context,
    exp_base: &el::Exp,
    exp_index: &el::Exp,
) -> Attempt<(il::ExpKind, il::TypKind)> {
    choose_sequential(
        ctx,
        |ctx| {
            let (exp_base, typ_base) = infer_exp(ctx, exp_base)?;
            let typ_element = as_list_typ(ctx, &typ_base)?;
            let typ_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_index.span);
            let exp_index = elab_exp(ctx, &typ_nat, exp_index)?;
            Ok((
                il::ExpKind::Idx(Box::new(exp_base), Box::new(exp_index)),
                typ_element.node,
            ))
        },
        |ctx| {
            let typ_text = typ_at(il::TypKind::Text, &exp_base.span);
            let exp_base = elab_exp(ctx, &typ_text, exp_base)?;
            let typ_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_index.span);
            let exp_index = elab_exp(ctx, &typ_nat, exp_index)?;
            Ok((
                il::ExpKind::Idx(Box::new(exp_base), Box::new(exp_index)),
                il::TypKind::Text,
            ))
        },
    )
}

fn infer_slice_exp(
    ctx: &mut Context,
    exp_base: &el::Exp,
    exp_index: &el::Exp,
    exp_length: &el::Exp,
) -> Attempt<(il::ExpKind, il::TypKind)> {
    choose_sequential(
        ctx,
        |ctx| {
            let (exp_base, typ_base) = infer_exp(ctx, exp_base)?;
            as_list_typ(ctx, &typ_base)?;
            let typ_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_index.span);
            let exp_index = elab_exp(ctx, &typ_nat, exp_index)?;
            let typ_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_length.span);
            let exp_length = elab_exp(ctx, &typ_nat, exp_length)?;
            Ok((
                il::ExpKind::Slice(
                    Box::new(exp_base),
                    Box::new(exp_index),
                    Box::new(exp_length),
                ),
                typ_base.node,
            ))
        },
        |ctx| {
            let typ_text = typ_at(il::TypKind::Text, &exp_base.span);
            let exp_base = elab_exp(ctx, &typ_text, exp_base)?;
            let typ_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_index.span);
            let exp_index = elab_exp(ctx, &typ_nat, exp_index)?;
            let typ_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_length.span);
            let exp_length = elab_exp(ctx, &typ_nat, exp_length)?;
            Ok((
                il::ExpKind::Slice(
                    Box::new(exp_base),
                    Box::new(exp_index),
                    Box::new(exp_length),
                ),
                il::TypKind::Text,
            ))
        },
    )
}

fn infer_dot_exp(
    ctx: &mut Context,
    exp: &el::Exp,
    atom: &el::Atom,
) -> Attempt<(il::ExpKind, il::TypKind)> {
    let (exp, typ) = infer_exp(ctx, exp)?;
    let fields = as_struct_typ(ctx, &typ)?;
    let Some((_, typ_field)) = fields
        .iter()
        .find(|(atom_field, _)| atom_field.node == atom.node)
    else {
        return fail_infer(atom.span.clone(), "field");
    };
    Ok((
        il::ExpKind::Dot(Box::new(exp), atom.clone()),
        typ_field.node.clone(),
    ))
}

fn infer_upd_exp(
    ctx: &mut Context,
    exp_base: &el::Exp,
    path: &el::Path,
    exp_field: &el::Exp,
) -> Attempt<(il::ExpKind, il::TypKind)> {
    let (exp_base, typ_base) = infer_exp(ctx, exp_base)?;
    let (path, typ_field) = elab_path(ctx, &typ_base, path)?;
    let exp_field = elab_exp(ctx, &typ_field, exp_field)?;
    Ok((
        il::ExpKind::Upd(Box::new(exp_base), path, Box::new(exp_field)),
        typ_base.node,
    ))
}

// - Call, iteration, and subtype expressions

fn infer_paren_exp(ctx: &mut Context, exp: &el::Exp) -> Attempt<(il::ExpKind, il::TypKind)> {
    let (exp, typ) = infer_exp(ctx, exp)?;
    Ok((exp.node.kind, typ.node))
}

fn infer_call_exp(
    ctx: &mut Context,
    span: &Span,
    id: &Id,
    targs: &[el::Targ],
    args: &[el::Arg],
) -> Attempt<(il::ExpKind, il::TypKind)> {
    let (tparams, params, typ_ret) = match ctx.find_func_signature(id) {
        Ok((tparams, params, typ_ret)) => (tparams.to_vec(), params.to_vec(), typ_ret.clone()),
        Err(error) => return fail(error),
    };
    if tparams.len() != targs.len() {
        return fail(arity_error(tparams.len(), targs.len(), id.span.clone()));
    }
    let mut targs_il = Vec::with_capacity(targs.len());
    for targ in targs {
        let targ_il = match elab_plain_typ(ctx, targ) {
            Ok(targ_il) => targ_il,
            Err(error) => return fail(error),
        };
        targs_il.push(targ_il);
    }
    let theta = match Theta::from_lists(&tparams, &targs_il) {
        Ok(theta) => theta,
        Err(mismatch) => {
            return fail(arity_error(
                mismatch.expected,
                mismatch.actual,
                id.span.clone(),
            ));
        }
    };
    let params = subst_params(&theta, &params)?;
    let typ_ret = subst_typ(&theta, &typ_ret)?;
    let args = elab_args(ctx, &params, args, false, span)?;
    Ok((il::ExpKind::Call(id.clone(), targs_il, args), typ_ret.node))
}

fn infer_iter_exp(
    ctx: &mut Context,
    exp: &el::Exp,
    iter: el::Iter,
) -> Attempt<(il::ExpKind, il::TypKind)> {
    let (exp, typ) = infer_exp(ctx, exp)?;
    let iter = elab_iter(iter);
    Ok((
        il::ExpKind::Iter(Box::new(exp), (iter, vec![])),
        il::TypKind::Iter(Box::new(typ), iter),
    ))
}

fn infer_sub_exp(
    ctx: &mut Context,
    exp: &el::Exp,
    plain_typ: &el::PlainTyp,
) -> Attempt<(il::ExpKind, il::TypKind)> {
    let (exp, typ_source) = infer_exp(ctx, exp)?;
    let typ_target = match elab_plain_typ(ctx, plain_typ) {
        Ok(typ) => typ,
        Err(error) => return fail(error),
    };
    let source_sub = sub_typ(&ctx.tdenv, &typ_source, &typ_target)?;
    let target_sub = sub_typ(&ctx.tdenv, &typ_target, &typ_source)?;
    if !source_sub && !target_sub {
        return fail_attempt(
            ElabErrorKind::TypeMismatch,
            exp.span.clone(),
            "subtype expression compares incomparable types",
        );
    }
    let check = optimize_sub_typ(&ctx.tdenv, &typ_source, &typ_target)?;
    Ok((
        il::ExpKind::Sub(Box::new(exp), typ_target, Box::new(check)),
        il::TypKind::Bool,
    ))
}

// - Expression elaboration

fn cast_exp(
    ctx: &Context,
    typ_expect: &il::Typ,
    typ_infer: &il::Typ,
    exp: il::Exp,
) -> Attempt<il::Exp> {
    let equivalent = equiv_typ(&ctx.tdenv, typ_expect, typ_infer)?;
    if equivalent {
        return Ok(exp);
    }
    let subtype = sub_typ(&ctx.tdenv, typ_infer, typ_expect)?;
    if subtype {
        let node = Noted::new(
            il::ExpKind::UpCast(typ_expect.clone(), Box::new(exp.clone())),
            typ_expect.node.clone(),
        );
        let exp = spanned!(node: node, span: exp.span);
        return Ok(exp);
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

fn elab_exp(ctx: &mut Context, typ_expect: &il::Typ, exp: &el::Exp) -> Attempt<il::Exp> {
    let error = ElabError::new(
        ElabErrorKind::NoMatchingAlternative,
        exp.span.clone(),
        "expression elaboration failed",
    );
    let parenthesized = matches!(exp.node, el::ExpKind::Paren(_));
    let span = exp.span.clone();
    elab_exp_inner(ctx, typ_expect, exp)
        .map(move |mut exp| {
            if parenthesized {
                respan_parenthesized_exp(&mut exp, &span);
            }
            exp
        })
        .map_err(|failure| failure.nest(error))
}

fn elab_exp_inner(ctx: &mut Context, typ_expect: &il::Typ, exp: &el::Exp) -> Attempt<il::Exp> {
    if let Ok((typ_base, iter)) = as_iter_typ(ctx, typ_expect) {
        return choose_sequential(
            ctx,
            |ctx| {
                if matches!(&exp.node, el::ExpKind::Var(id) if id.node == "_")
                    || matches!(&exp.node, el::ExpKind::Eps)
                    || matches!(&exp.node, el::ExpKind::List(exps) if exps.is_empty())
                {
                    return fail_silent();
                }
                let exp_inner = elab_exp(ctx, &typ_base, exp)?;
                let kind = match iter {
                    il::Iter::Opt => il::ExpKind::Opt(Some(Box::new(exp_inner))),
                    il::Iter::List => il::ExpKind::List(vec![exp_inner]),
                };
                let exp_il = Noted::new(kind, typ_expect.node.clone());
                let exp_il = spanned!(node: exp_il, span: exp.span.clone());
                Ok(exp_il)
            },
            |ctx| elab_exp_normal(ctx, typ_expect, exp),
        );
    }
    elab_exp_normal(ctx, typ_expect, exp)
}

fn elab_exp_normal(ctx: &mut Context, typ_expect: &il::Typ, exp: &el::Exp) -> Attempt<il::Exp> {
    let checkpoint = ctx.checkpoint();
    match infer_exp(ctx, exp) {
        Ok((exp, typ_infer)) => match cast_exp(ctx, typ_expect, &typ_infer, exp) {
            Ok(exp) => {
                ctx.commit(checkpoint);
                Ok(exp)
            }
            Err(failure) => {
                ctx.rollback(checkpoint);
                Err(failure)
            }
        },
        Err(_) => {
            ctx.rollback(checkpoint);
            elab_exp_contextual(ctx, typ_expect, exp)
        }
    }
}

// - Paths

fn elab_path(
    ctx: &mut Context,
    typ_expect: &il::Typ,
    path: &el::Path,
) -> Attempt<(il::Path, il::Typ)> {
    match &path.node {
        el::PathKind::Root => {
            let path_il = Noted::new(il::PathKind::Root, typ_expect.node.clone());
            let path_il = spanned!(node: path_il, span: path.span.clone());
            let typ = spanned!(node: typ_expect.node.clone(), span: path.span.clone());
            Ok((path_il, typ))
        }
        el::PathKind::Idx(path_inner, exp_index) => choose_sequential(
            ctx,
            |ctx| {
                let (path_inner, typ_inner) = elab_path(ctx, typ_expect, path_inner)?;
                let typ_element = as_list_typ(ctx, &typ_inner)?;
                let typ_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_index.span);
                let index = elab_exp(ctx, &typ_nat, exp_index)?;
                let path_kind = il::PathKind::Idx(Box::new(path_inner), Box::new(index));
                let path_il = Noted::new(path_kind, typ_element.node.clone());
                let path_il = spanned!(node: path_il, span: path.span.clone());
                let typ_element = spanned!(node: typ_element.node, span: path.span.clone());
                Ok((path_il, typ_element))
            },
            |ctx| {
                let (path_inner, typ_inner) = elab_path(ctx, typ_expect, path_inner)?;
                as_text_typ(ctx, &typ_inner)?;
                let typ_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_index.span);
                let index = elab_exp(ctx, &typ_nat, exp_index)?;
                let path_kind = il::PathKind::Idx(Box::new(path_inner), Box::new(index));
                let path_il = Noted::new(path_kind, typ_inner.node.clone());
                let path_il = spanned!(node: path_il, span: path.span.clone());
                let typ_inner = spanned!(node: typ_inner.node, span: path.span.clone());
                Ok((path_il, typ_inner))
            },
        ),
        el::PathKind::Slice(path_inner, exp_index, exp_length) => {
            let (path_inner, typ_inner) = elab_path(ctx, typ_expect, path_inner)?;
            let is_list = as_list_typ(ctx, &typ_inner).is_ok();
            let is_text = as_text_typ(ctx, &typ_inner).is_ok();
            if !is_list && !is_text {
                return fail_attempt(
                    ElabErrorKind::CannotDestructure(TypeShape::List),
                    typ_inner.span,
                    "slice path requires a list or text",
                );
            }
            let typ_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_index.span);
            let exp_index = elab_exp(ctx, &typ_nat, exp_index)?;
            let typ_nat = typ_at(il::TypKind::Num(xl::num::Typ::Nat), &exp_length.span);
            let exp_length = elab_exp(ctx, &typ_nat, exp_length)?;
            let path_kind = il::PathKind::Slice(
                Box::new(path_inner),
                Box::new(exp_index),
                Box::new(exp_length),
            );
            let path_il = Noted::new(path_kind, typ_inner.node.clone());
            let path_il = spanned!(node: path_il, span: path.span.clone());
            let typ_inner = spanned!(node: typ_inner.node, span: path.span.clone());
            Ok((path_il, typ_inner))
        }
        el::PathKind::Dot(path_inner, atom) => {
            let (path_inner, typ_inner) = elab_path(ctx, typ_expect, path_inner)?;
            let fields = as_struct_typ(ctx, &typ_inner)?;
            let Some((_, typ_field)) = fields
                .into_iter()
                .find(|(atom_field, _)| atom_field.node == atom.node)
            else {
                return fail_infer(atom.span.clone(), "field");
            };
            let path_kind = il::PathKind::Dot(Box::new(path_inner), atom.clone());
            let path_il = Noted::new(path_kind, typ_field.node.clone());
            let path_il = spanned!(node: path_il, span: path.span.clone());
            let typ_field = spanned!(node: typ_field.node, span: path.span.clone());
            Ok((path_il, typ_field))
        }
    }
}

// - Parameters and arguments

fn elab_param(ctx: &mut Context, param: &el::Param) -> Result<il::Param, ElabError> {
    let kind = match &param.node {
        el::ParamKind::Exp(typ) => {
            let typ = elab_plain_typ(ctx, typ)?;
            il::ParamKind::Exp(typ)
        }
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
            let (params, typ_ret) = {
                let mut ctx = ctx.scope();
                ctx.add_tparams(tparams)?;
                let params = params
                    .iter()
                    .map(|param| elab_param(&mut ctx, param))
                    .collect::<Result<Vec<_>, _>>()?;
                let typ_ret = elab_plain_typ(&ctx, typ_ret)?;
                (params, typ_ret)
            };
            il::ParamKind::Def(id.clone(), tparams.clone(), params, typ_ret)
        }
    };
    let param = spanned!(node: kind, span: param.span.clone());
    Ok(param)
}

fn typ_of_param(param: &il::Param) -> il::Typ {
    match &param.node {
        il::ParamKind::Exp(typ) => typ.clone(),
        il::ParamKind::Def(_, tparams, params, typ_ret) => {
            let func_typ = il::FuncTyp {
                tparams: tparams.clone(),
                typs_params: params.iter().map(typ_of_param).collect(),
                typ_ret: Box::new(typ_ret.clone()),
            };
            let typ = il::TypKind::Func(func_typ);
            spanned!(node: typ, span: param.span.clone())
        }
    }
}

fn elab_arg(ctx: &mut Context, param: &il::Param, arg: &el::Arg, as_def: bool) -> Attempt<il::Arg> {
    match (&param.node, &arg.node) {
        (il::ParamKind::Exp(typ), el::ArgKind::Exp(exp)) => {
            let exp = elab_exp(ctx, typ, exp)?;
            let arg_il = il::ArgKind::Exp(Box::new(exp));
            let arg_il = spanned!(node: arg_il, span: arg.span.clone());
            Ok(arg_il)
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
            if let Err(error) = ctx.add_defined_func(
                id_param.clone(),
                tparams.clone(),
                params.clone(),
                typ_ret.clone(),
            ) {
                return fail(error);
            }
            let arg_il = il::ArgKind::Def(id_arg.clone());
            let arg_il = spanned!(node: arg_il, span: arg.span.clone());
            Ok(arg_il)
        }
        (il::ParamKind::Def(_, tparams, params, typ_ret), el::ArgKind::Def(id_arg)) => {
            let (tparams_arg, params_arg, typ_ret_arg) = match ctx.find_func_signature(id_arg) {
                Ok(signature) => signature,
                Err(error) => return fail(error),
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
            let equivalent = equiv_func_typ(&ctx.tdenv, &arg.span, &typ_param, &typ_arg)?;
            if !equivalent {
                return fail_attempt(
                    ElabErrorKind::InvalidArgument,
                    arg.span.clone(),
                    "function argument type does not match",
                );
            }
            let arg_il = il::ArgKind::Def(id_arg.clone());
            let arg_il = spanned!(node: arg_il, span: arg.span.clone());
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
    params: &[il::Param],
    args: &[el::Arg],
    as_def: bool,
    span: &Span,
) -> Attempt<Vec<il::Arg>> {
    if params.len() != args.len() {
        return fail_attempt(
            ElabErrorKind::ArityMismatch,
            span.clone(),
            "argument count does not match parameter count",
        );
    }
    let mut args_il = Vec::with_capacity(args.len());
    for (param, arg) in params.iter().zip(args) {
        let arg = elab_arg(ctx, param, arg, as_def)?;
        args_il.push(arg);
    }
    Ok(args_il)
}

// == Premises

#[derive(Clone, Debug)]
enum PremInternal {
    Some(il::Prem),
    Var(Span),
    Else(Span),
}

fn elab_relation_prem(
    ctx: &mut Context,
    prem_span: &Span,
    id: &Id,
    exp: &el::Exp,
    negated: bool,
) -> Attempt<PremInternal> {
    let (not_typ, input_hint) = match ctx.find_rel_signature(id) {
        Ok((not_typ, input_hint)) => (not_typ.clone(), input_hint.clone()),
        Err(error) => return fail(error),
    };
    let not_exp = elab_not_exp(ctx, &not_typ, exp)?;
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
    let prem = spanned!(node: kind, span: prem_span.clone());
    Ok(PremInternal::Some(prem))
}

fn elab_prem(ctx: &mut Context, prem: &el::Prem) -> Attempt<PremInternal> {
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
            let typ = match elab_plain_typ(ctx, &var_prem.plain_typ) {
                Ok(typ) => typ,
                Err(error) => return fail(error),
            };
            if let Err(error) = ctx.add_metavar(var_prem.id.clone(), typ) {
                return fail(error);
            }
            Ok(PremInternal::Var(prem.span.clone()))
        }
        el::PremKind::Rule(rule_prem) => {
            elab_relation_prem(ctx, &prem.span, &rule_prem.id, &rule_prem.exp, false)
        }
        el::PremKind::RuleNot(rule_prem) => {
            elab_relation_prem(ctx, &prem.span, &rule_prem.id, &rule_prem.exp, true)
        }
        el::PremKind::If(if_prem) => {
            let typ_bool = typ_at(il::TypKind::Bool, &if_prem.exp.span);
            let exp = elab_exp(ctx, &typ_bool, &if_prem.exp)?;
            let prem_kind = il::PremKind::If(il::IfPrem { exp });
            let prem = spanned!(node: prem_kind, span: prem.span.clone());
            Ok(PremInternal::Some(prem))
        }
        el::PremKind::Else => Ok(PremInternal::Else(prem.span.clone())),
        el::PremKind::Iter(iter_prem) => {
            let prem_inner = elab_prem(ctx, &iter_prem.prem)?;
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
            let iterated = il::IteratedPrem {
                prem: Box::new(prem_inner),
                iter_prem,
            };
            let prem_kind = il::PremKind::Iter(iterated);
            let prem = spanned!(node: prem_kind, span: prem.span.clone());
            Ok(PremInternal::Some(prem))
        }
        el::PremKind::Debug(debug_prem) => {
            let (exp, _) = infer_exp(ctx, &debug_prem.exp)?;
            let prem_kind = il::PremKind::Debug(il::DebugPrem { exp });
            let prem = spanned!(node: prem_kind, span: prem.span.clone());
            Ok(PremInternal::Some(prem))
        }
    }
}

fn elab_prems(
    ctx: &mut Context,
    prems: &[el::Prem],
    span: &Span,
) -> Attempt<(Vec<il::Prem>, bool)> {
    let mut prems_il = Vec::new();
    let mut else_count = 0;
    for prem in prems {
        let prem = elab_prem(ctx, prem)?;
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
    Ok((prems_il, else_count == 1))
}

// == Contextual expression elaboration

fn elab_not_exp(ctx: &mut Context, not_typ: &il::NotTyp, exp: &el::Exp) -> Attempt<il::NotExp> {
    if let el::ExpKind::Paren(exp) = &exp.node {
        return elab_not_exp(ctx, not_typ, exp);
    }
    match (&not_typ.node, &exp.node) {
        (Mixfix::Arg(typ), _) => {
            let exp = elab_exp(ctx, typ, exp)?;
            Ok(Mixfix::Arg(exp))
        }
        (Mixfix::Atom(atom_expect), el::ExpKind::Atom(atom)) if atom_expect.node == atom.node => {
            Ok(Mixfix::Atom(atom_expect.clone()))
        }
        (Mixfix::Seq(not_typs), el::ExpKind::Seq(exps)) => {
            if not_typs.len() != exps.len() {
                return fail_attempt(
                    ElabErrorKind::NoMatchingAlternative,
                    exp.span.clone(),
                    "notation sequence arity does not match",
                );
            }
            let mut not_exps = Vec::with_capacity(exps.len());
            for (not_typ_inner, exp) in not_typs.iter().zip(exps) {
                let not_typ_inner =
                    spanned!(node: not_typ_inner.clone(), span: not_typ.span.clone());
                let not_exp = elab_not_exp(ctx, &not_typ_inner, exp)?;
                not_exps.push(not_exp);
            }
            Ok(Mixfix::Seq(not_exps))
        }
        (
            Mixfix::Infix(not_typ_l, atom_expect, not_typ_r),
            el::ExpKind::Infix(exp_l, atom, exp_r),
        ) if atom_expect.node == atom.node => {
            let not_typ_l = spanned!(node: (**not_typ_l).clone(), span: not_typ.span.clone());
            let not_typ_r = spanned!(node: (**not_typ_r).clone(), span: not_typ.span.clone());
            let exp_l = elab_not_exp(ctx, &not_typ_l, exp_l)?;
            let exp_r = elab_not_exp(ctx, &not_typ_r, exp_r)?;
            Ok(Mixfix::Infix(
                Box::new(exp_l),
                atom_expect.clone(),
                Box::new(exp_r),
            ))
        }
        (
            Mixfix::Brack(atom_expect_l, not_typ_inner, atom_expect_r),
            el::ExpKind::Brack(atom_l, exp_inner, atom_r),
        ) if atom_expect_l.node == atom_l.node && atom_expect_r.node == atom_r.node => {
            let not_typ_inner =
                spanned!(node: (**not_typ_inner).clone(), span: not_typ.span.clone());
            let exp_inner = elab_not_exp(ctx, &not_typ_inner, exp_inner)?;
            Ok(Mixfix::Brack(
                atom_expect_l.clone(),
                Box::new(exp_inner),
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

fn elab_struct_exp(
    ctx: &mut Context,
    typ_expect: &il::Typ,
    typ_fields: &[il::TypField],
    exp: &el::Exp,
) -> Attempt<il::Exp> {
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
        let exp_field = elab_exp(ctx, typ, exp_field)?;
        fields.push((atom_expect.clone(), exp_field));
    }
    let span = exp.span.clone();
    let exp = Noted::new(il::ExpKind::Str(fields), typ_expect.node.clone());
    let exp = spanned!(node: exp, span: span);
    Ok(exp)
}

fn elab_variant_exp(
    ctx: &mut Context,
    typ_expect: &il::Typ,
    typ_cases: &[il::TypCase],
    exp: &el::Exp,
) -> Attempt<il::Exp> {
    let checkpoint = ctx.checkpoint();
    let mut matches = Vec::new();
    for (not_typ, origin, _) in typ_cases {
        let candidate = ctx.checkpoint();
        let not_exp = match elab_not_exp(ctx, not_typ, exp) {
            Ok(not_exp) => not_exp,
            Err(_) => {
                ctx.rollback(candidate);
                continue;
            }
        };
        let typ_case = il::TypKind::Var(origin.node.0.clone(), origin.node.1.clone());
        let typ_case = spanned!(node: typ_case, span: origin.span.clone());
        let exp_case = il::ExpKind::Case(Box::new(not_exp));
        let exp_case = Noted::new(exp_case, typ_case.node.clone());
        let exp_case = spanned!(node: exp_case, span: exp.span.clone());
        let exp_case = match cast_exp(ctx, typ_expect, &typ_case, exp_case) {
            Ok(exp_case) => exp_case,
            Err(_) => {
                ctx.rollback(candidate);
                continue;
            }
        };
        ctx.commit(candidate);
        matches.push(exp_case);
    }
    match matches.len() {
        1 => {
            ctx.commit(checkpoint);
            Ok(matches.pop().expect("single variant match"))
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

fn elab_exp_contextual(ctx: &mut Context, typ_expect: &il::Typ, exp: &el::Exp) -> Attempt<il::Exp> {
    if matches!(&exp.node, el::ExpKind::Var(id) if id.node == "_") {
        let var =
            il_fresh::var_from_typ_wildcard(&ctx.menv, &ctx.frees, exp.span.clone(), typ_expect);
        let exp = il_var::as_exp(false, &var);
        ctx.add_free(var.id);
        return Ok(exp);
    }
    if let il::TypKind::Var(id, targs) = &typ_expect.node
        && let Some(TypeDef::Defined(tparams, def_typ)) = ctx.find_typdef_opt(id)
        && let il::DefTypKind::Plain(typ) = &def_typ.node
    {
        let theta = match Theta::from_lists(tparams, targs) {
            Ok(theta) => theta,
            Err(mismatch) => {
                return fail(arity_error(
                    mismatch.expected,
                    mismatch.actual,
                    typ_expect.span.clone(),
                ));
            }
        };
        let typ = subst_typ(&theta, typ)?;
        return elab_exp_normal(ctx, &typ, exp);
    }
    match &exp.node {
        el::ExpKind::Eps => {
            let (_, iter) = as_iter_typ(ctx, typ_expect)?;
            let kind = match iter {
                il::Iter::Opt => il::ExpKind::Opt(None),
                il::Iter::List => il::ExpKind::List(vec![]),
            };
            let span = exp.span.clone();
            let exp = Noted::new(kind, typ_expect.node.clone());
            let exp = spanned!(node: exp, span: span);
            Ok(exp)
        }
        el::ExpKind::List(exps) => {
            let (typ_base, iter) = as_iter_typ(ctx, typ_expect)?;
            if iter != il::Iter::List {
                return fail_attempt(
                    ElabErrorKind::InvalidIteration,
                    exp.span.clone(),
                    "list expression has optional expected type",
                );
            }
            let mut exps_il = Vec::with_capacity(exps.len());
            for exp in exps {
                let exp = elab_exp(ctx, &typ_base, exp)?;
                exps_il.push(exp);
            }
            let span = exp.span.clone();
            let exp = Noted::new(il::ExpKind::List(exps_il), typ_expect.node.clone());
            let exp = spanned!(node: exp, span: span);
            Ok(exp)
        }
        el::ExpKind::Cons(exp_head, exp_tail) => {
            let (typ_base, iter) = as_iter_typ(ctx, typ_expect)?;
            let exp_head = elab_exp(ctx, &typ_base, exp_head)?;
            let typ_tail = il::TypKind::Iter(Box::new(typ_base), iter);
            let typ_tail = spanned!(node: typ_tail, span: typ_expect.span.clone());
            let exp_tail = elab_exp(ctx, &typ_tail, exp_tail)?;
            let kind = il::ExpKind::Cons(Box::new(exp_head), Box::new(exp_tail));
            let span = exp.span.clone();
            let exp = Noted::new(kind, typ_expect.node.clone());
            let exp = spanned!(node: exp, span: span);
            Ok(exp)
        }
        el::ExpKind::Cat(exp_l, exp_r) => {
            let kind = choose_sequential(
                ctx,
                |ctx| {
                    let (typ_base, iter) = as_iter_typ(ctx, typ_expect)?;
                    let typ_iter_kind = il::TypKind::Iter(Box::new(typ_base.clone()), iter);
                    let typ_iter = spanned!(node: typ_iter_kind, span: typ_base.span);
                    let exp_l = elab_exp(ctx, &typ_iter, exp_l)?;
                    let exp_r = elab_exp(ctx, &typ_iter, exp_r)?;
                    Ok(il::ExpKind::Cat(Box::new(exp_l), Box::new(exp_r)))
                },
                |ctx| {
                    let typ_text = typ_at(il::TypKind::Text, &exp_l.span);
                    let exp_l = elab_exp(ctx, &typ_text, exp_l)?;
                    let typ_text = typ_at(il::TypKind::Text, &exp_r.span);
                    let exp_r = elab_exp(ctx, &typ_text, exp_r)?;
                    Ok(il::ExpKind::Cat(Box::new(exp_l), Box::new(exp_r)))
                },
            )?;
            let span = exp.span.clone();
            let exp = Noted::new(kind, typ_expect.node.clone());
            let exp = spanned!(node: exp, span: span);
            Ok(exp)
        }
        el::ExpKind::Tuple(exps) => {
            let typs = as_tuple_typ(ctx, typ_expect)?;
            if typs.len() != exps.len() {
                return fail_attempt(
                    ElabErrorKind::ArityMismatch,
                    exp.span.clone(),
                    "tuple expression arity does not match",
                );
            }
            let mut exps_il = Vec::with_capacity(exps.len());
            for (typ, exp) in typs.iter().zip(exps) {
                let exp = elab_exp(ctx, typ, exp)?;
                exps_il.push(exp);
            }
            let span = exp.span.clone();
            let exp = Noted::new(il::ExpKind::Tuple(exps_il), typ_expect.node.clone());
            let exp = spanned!(node: exp, span: span);
            Ok(exp)
        }
        el::ExpKind::Paren(exp_inner) => {
            let exp_inner = elab_exp(ctx, typ_expect, exp_inner)?;
            let span = exp.span.clone();
            let exp = Noted::new(exp_inner.node.kind, exp_inner.node.note);
            let exp = spanned!(node: exp, span: span);
            Ok(exp)
        }
        el::ExpKind::Iter(exp_inner, iter) => {
            let (typ_base, iter_expect) = as_iter_typ(ctx, typ_expect)?;
            let iter = elab_iter(*iter);
            if iter != iter_expect {
                return fail_attempt(
                    ElabErrorKind::InvalidIteration,
                    exp.span.clone(),
                    "iteration mismatch",
                );
            }
            let exp_inner = elab_exp(ctx, &typ_base, exp_inner)?;
            let kind = il::ExpKind::Iter(Box::new(exp_inner), (iter, vec![]));
            let span = exp.span.clone();
            let exp = Noted::new(kind, typ_expect.node.clone());
            let exp = spanned!(node: exp, span: span);
            Ok(exp)
        }
        _ => {
            if let il::TypKind::Var(id, targs) = &typ_expect.node {
                if let Some(TypeDef::Defined(tparams, def_typ)) = ctx.find_typdef_opt(id) {
                    let theta = match Theta::from_lists(tparams, targs) {
                        Ok(theta) => theta,
                        Err(mismatch) => {
                            return fail(arity_error(
                                mismatch.expected,
                                mismatch.actual,
                                typ_expect.span.clone(),
                            ));
                        }
                    };
                    if let il::DefTypKind::Struct(fields) = &def_typ.node {
                        let mut fields_subst = Vec::with_capacity(fields.len());
                        for (atom, typ) in fields {
                            let typ = subst_typ(&theta, typ)?;
                            fields_subst.push((atom.clone(), typ));
                        }
                        return elab_struct_exp(ctx, typ_expect, &fields_subst, exp);
                    }
                    if let il::DefTypKind::Variant(cases) = &def_typ.node {
                        let mut cases_subst = Vec::with_capacity(cases.len());
                        for (not_typ, origin, hints) in cases {
                            let not_typ = subst_not_typ(&theta, not_typ)?;
                            let targs = subst_typs(&theta, &origin.node.1)?;
                            let origin = spanned! {
                                node: (origin.node.0.clone(), targs),
                                span: origin.span.clone(),
                            };
                            cases_subst.push((not_typ, origin, hints.clone()));
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

// == Rules, clauses, and definitions

// - Definition validation

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

// - Rules and clauses

fn elab_rule(
    ctx: &mut Context,
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
    let mut ctx = ctx.scope();
    ctx.reset_frees();
    let frees = rule.free();
    ctx.add_frees(&frees);
    let not_exp = finish(elab_not_exp(&mut ctx, not_typ, exp))?;
    let (prems, is_else) = finish(elab_prems(&mut ctx, prems, &ruleid.span))?;
    let rule_kind = il::RuleKind {
        id: ruleid.clone(),
        not_exp,
        prems,
    };
    let rule = spanned!(node: rule_kind, span: rule.span.clone());
    Ok((rule, is_else))
}

fn elab_rule_group(
    ctx: &mut Context,
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
        0 => {
            let group = (groupid.clone(), rules_il);
            let group = spanned!(node: group, span: span.clone());
            Ok((Some(group), None))
        }
        1 if rules.len() == 1 => {
            let group = (groupid.clone(), else_rules.remove(0));
            let group = spanned!(node: group, span: span.clone());
            Ok((None, Some(group)))
        }
        _ => Err(ElabError::new(
            ElabErrorKind::InvalidRule,
            span.clone(),
            "invalid otherwise rule group",
        )),
    }
}

fn elab_clause(
    ctx: &mut Context,
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
    let func_def = el::FuncDef {
        id: id.clone(),
        tparams: tparams.to_vec(),
        args: args.to_vec(),
        exp: exp.clone(),
        prems: prems.to_vec(),
    };
    let def_kind = el::DefKind::FuncDef(func_def);
    let def = spanned!(node: def_kind, span: span.clone());
    let mut ctx = ctx.scope();
    ctx.reset_frees();
    let frees = def.free();
    ctx.add_frees(&frees);
    ctx.add_tparams(tparams)?;
    let args = finish(elab_args(&mut ctx, &params, args, true, span))?;
    let (premises, is_else) = finish(elab_prems(&mut ctx, prems, span))?;
    let expression = finish(elab_exp(&mut ctx, &typ_ret, exp))?;
    let clause_kind = il::ClauseKind {
        args,
        expression,
        premises,
    };
    let clause = spanned!(node: clause_kind, span: span.clone());
    Ok((clause, is_else))
}

// - Definitions

fn elab_extern_syntax_def(
    ctx: &mut Context,
    span: Span,
    def: &el::ExternSyntaxDef,
) -> Result<il::Def, ElabError> {
    if !valid_tid(&def.id) {
        return Err(ElabError::new(
            ElabErrorKind::InvalidIdentifier,
            def.id.span.clone(),
            "invalid type identifier",
        ));
    }
    ctx.add_typdef(def.id.clone(), TypeDef::Extern)?;
    let typ = il::TypKind::Var(def.id.clone(), vec![]);
    let typ = spanned!(node: typ, span: def.id.span.clone());
    ctx.add_metavar(def.id.clone(), typ)?;
    let extern_typ = il::ExternTyp {
        id: def.id.clone(),
        hints: def.hints.clone(),
    };
    let def_kind = il::DefKind::ExternTyp(extern_typ);
    let def_il = spanned!(node: def_kind, span: span);
    Ok(def_il)
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
            let typ = il::TypKind::Var(entry.id.clone(), vec![]);
            let typ = spanned!(node: typ, span: entry.id.span.clone());
            ctx.add_metavar(entry.id.clone(), typ)?;
        }
    }
    Ok(())
}

fn elab_typ_def(ctx: &mut Context, def: &el::TypDef) -> Result<il::Def, ElabError> {
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
                let typ = il::TypKind::Var(def.id.clone(), vec![]);
                let typ = spanned!(node: typ, span: def.id.span.clone());
                ctx.add_metavar(def.id.clone(), typ)?;
            }
        }
    }
    let (typdef, def_typ) = {
        let mut ctx = ctx.scope();
        ctx.add_tparams(&def.tparams)?;
        elab_def_typ(&ctx, &def.id, &def.tparams, &def.def_typ)?
    };
    ctx.update_typdef(&def.id, typdef)?;
    let typ_def = il::TypDef {
        id: def.id.clone(),
        tparams: def.tparams.clone(),
        def_typ,
        hints: def.hints.clone(),
    };
    let def_kind = il::DefKind::Typ(typ_def);
    let def_il = spanned!(node: def_kind, span: def.def_typ.span.clone());
    Ok(def_il)
}

fn elab_var_def(ctx: &mut Context, def: &el::VarDef) -> Result<il::Def, ElabError> {
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
    let typ = elab_plain_typ(ctx, &def.plain_typ)?;
    ctx.add_metavar(def.id.clone(), typ.clone())?;
    let var_def = il::VarDef {
        id: def.id.clone(),
        typ,
        hints: def.hints.clone(),
    };
    let def_kind = il::DefKind::Var(var_def);
    let def_il = spanned!(node: def_kind, span: def.id.span.clone());
    Ok(def_il)
}

fn elab_rel_def(
    ctx: &mut Context,
    span: Span,
    id: &Id,
    not_typ: &el::NotTyp,
    hints: &[el::Hint],
    is_extern: bool,
) -> Result<il::Def, ElabError> {
    let typ = el::Typ::Notation(not_typ.clone());
    let not_typ = elab_not_typ(ctx, &typ)?;
    let input_hint = fetch_input_hint(&span, &not_typ, hints)?;
    let kind = if is_extern {
        ctx.add_extern_rel(id.clone(), not_typ.clone(), input_hint.clone())?;
        let rel = il::ExternRel {
            id: id.clone(),
            not_typ,
            input_hint,
            hints: hints.to_vec(),
        };
        il::DefKind::ExternRel(rel)
    } else {
        ctx.add_defined_rel(id.clone(), not_typ.clone(), input_hint.clone())?;
        let rel = il::Rel {
            id: id.clone(),
            not_typ,
            input_hint,
            rule_groups: vec![],
            else_group: None,
            hints: hints.to_vec(),
        };
        il::DefKind::Rel(rel)
    };
    let def = spanned!(node: kind, span: span);
    Ok(def)
}

#[derive(Clone, Copy)]
enum DecKind {
    Extern,
    Builtin,
    Defined,
}

fn elab_func_dec(
    ctx: &mut Context,
    span: Span,
    id: &Id,
    tparams: &[el::TParam],
    params: &[el::Param],
    plain_typ: &el::PlainTyp,
    hints: &[el::Hint],
    kind: DecKind,
) -> Result<il::Def, ElabError> {
    distinct_tparams(tparams, &id.span)?;
    let (params_il, typ) = {
        let mut ctx = ctx.scope();
        ctx.add_tparams(tparams)?;
        let params_il = params
            .iter()
            .map(|param| elab_param(&mut ctx, param))
            .collect::<Result<Vec<_>, _>>()?;
        let typ = elab_plain_typ(&ctx, plain_typ)?;
        (params_il, typ)
    };
    let def_kind = match kind {
        DecKind::Extern => {
            ctx.add_extern_func(id.clone(), tparams.to_vec(), params_il.clone(), typ.clone())?;
            let func = il::ExternDec {
                id: id.clone(),
                tparams: tparams.to_vec(),
                params: params_il,
                typ,
                hints: hints.to_vec(),
            };
            il::DefKind::ExternDec(func)
        }
        DecKind::Builtin => {
            ctx.add_builtin_func(id.clone(), tparams.to_vec(), params_il.clone(), typ.clone())?;
            let func = il::BuiltinDec {
                id: id.clone(),
                tparams: tparams.to_vec(),
                params: params_il,
                typ,
                hints: hints.to_vec(),
            };
            il::DefKind::BuiltinDec(func)
        }
        DecKind::Defined => {
            ctx.add_defined_func(id.clone(), tparams.to_vec(), params_il.clone(), typ.clone())?;
            let func = il::FuncDec {
                id: id.clone(),
                tparams: tparams.to_vec(),
                params: params_il,
                typ,
                clauses: vec![],
                else_clause: None,
                hints: hints.to_vec(),
            };
            il::DefKind::FuncDec(func)
        }
    };
    let def = spanned!(node: def_kind, span: span);
    Ok(def)
}

fn elab_table_dec(
    ctx: &mut Context,
    span: Span,
    def: &el::TableDecDef,
) -> Result<il::Def, ElabError> {
    let params = def
        .params
        .iter()
        .map(|param| elab_param(ctx, param))
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
    let typ = elab_plain_typ(ctx, &def.plain_typ)?;
    if typ.node != il::TypKind::Bool {
        return Err(ElabError::new(
            ElabErrorKind::TypeMismatch,
            typ.span,
            "table must return boolean",
        ));
    }
    ctx.add_table_func(def.id.clone(), params.clone(), typ.clone())?;
    let table_dec = il::TableDec {
        id: def.id.clone(),
        params,
        typ,
        rows: vec![],
        hints: def.hints.clone(),
    };
    let def_kind = il::DefKind::TableDec(table_dec);
    let def_il = spanned!(node: def_kind, span: span);
    Ok(def_il)
}

fn elab_table_def(ctx: &mut Context, def: &el::TableDef) -> Result<(), ElabError> {
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
                let arg = el::ArgKind::Exp(Box::new(exp));
                spanned!(node: arg, span: span)
            })
            .collect::<Vec<_>>();
        let (args, exp_body) = {
            let mut ctx = ctx.scope();
            ctx.reset_frees();
            let frees = row.free();
            ctx.add_frees(&frees);
            let args = finish(elab_args(&mut ctx, &params, &args, true, &row.span))?;
            let exp_body = finish(elab_exp(&mut ctx, &typ, exp_body))?;
            (args, exp_body)
        };
        let row = spanned!(node: (args, exp_body), span: row.span.clone());
        rows.push(row);
    }
    ctx.add_table_func_rows(&def.id, rows)?;
    Ok(())
}

fn elab_def(ctx: &mut Context, def: &el::Def) -> Result<Option<il::Def>, ElabError> {
    let span = def.span.clone();
    match &def.node {
        el::DefKind::ExternSyntax(def) => {
            let def = elab_extern_syntax_def(ctx, span, def)?;
            Ok(Some(def))
        }
        el::DefKind::Syntax(def) => {
            elab_syntax_def(ctx, def)?;
            Ok(None)
        }
        el::DefKind::Typ(def) => {
            let def = elab_typ_def(ctx, def)?;
            Ok(Some(def))
        }
        el::DefKind::Var(def) => {
            let def = elab_var_def(ctx, def)?;
            Ok(Some(def))
        }
        el::DefKind::ExternRel(def) => {
            let def = elab_rel_def(ctx, span, &def.id, &def.not_typ, &def.hints, true)?;
            Ok(Some(def))
        }
        el::DefKind::Rel(def) => {
            let def = elab_rel_def(ctx, span, &def.id, &def.not_typ, &def.hints, false)?;
            Ok(Some(def))
        }
        el::DefKind::RuleGroup(def) => {
            let (rule_group, else_group) =
                elab_rule_group(ctx, &span, &def.relid, &def.groupid, &def.rules)?;
            if let Some(rule_group) = rule_group {
                ctx.add_defined_rule_group(&def.relid, rule_group)?;
            }
            if let Some(else_group) = else_group {
                ctx.add_defined_else_group(&def.relid, else_group)?;
            }
            Ok(None)
        }
        el::DefKind::ExternDec(def) => {
            let def = elab_func_dec(
                ctx,
                span,
                &def.id,
                &def.tparams,
                &def.params,
                &def.plain_typ,
                &def.hints,
                DecKind::Extern,
            )?;
            Ok(Some(def))
        }
        el::DefKind::BuiltinDec(def) => {
            let def = elab_func_dec(
                ctx,
                span,
                &def.id,
                &def.tparams,
                &def.params,
                &def.plain_typ,
                &def.hints,
                DecKind::Builtin,
            )?;
            Ok(Some(def))
        }
        el::DefKind::TableDec(def) => {
            let def = elab_table_dec(ctx, span, def)?;
            Ok(Some(def))
        }
        el::DefKind::FuncDec(def) => {
            let def = elab_func_dec(
                ctx,
                span,
                &def.id,
                &def.tparams,
                &def.params,
                &def.plain_typ,
                &def.hints,
                DecKind::Defined,
            )?;
            Ok(Some(def))
        }
        el::DefKind::TableDef(def) => {
            elab_table_def(ctx, def)?;
            Ok(None)
        }
        el::DefKind::FuncDef(def) => {
            let (clause, is_else) = elab_clause(
                ctx,
                &span,
                &def.id,
                &def.tparams,
                &def.args,
                &def.exp,
                &def.prems,
            )?;
            if is_else {
                ctx.add_defined_func_else_clause(&def.id, clause)?;
            } else {
                ctx.add_defined_func_clause(&def.id, clause)?;
            }
            Ok(None)
        }
        el::DefKind::Sep => Ok(None),
    }
}

// - Definition population

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
            let def = spanned!(node: kind, span: def.span);
            Ok(def)
        })
        .collect()
}

// == Entry point

pub(super) fn elaborate(spec: &el::Spec) -> Result<il::Spec, ElabError> {
    let mut ctx = Context::new();
    let mut defs = Vec::new();
    for def in spec {
        if let Some(def) = elab_def(&mut ctx, def)? {
            defs.push(def);
        }
    }
    let defs = populate_defs(&ctx, defs)?;
    dimension::analyze_spec(&defs)
}
