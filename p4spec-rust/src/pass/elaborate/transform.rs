//! Elaboration-language validation and conversion to internal syntax
//!
//! `elaborate` walks EL definitions in source order
//! and builds the `Context` as declarations appear;
//!
//! `populate_defs` then moves the collected rule groups and clauses
//! into their relation and function definitions;
//!
//! `dimension::analyze_spec` annotates iterations.
//!
//! Expressions elaborate bidirectionally:
//! `infer_exp` synthesizes a type bottom-up,
//! while `elab_exp` checks against an expected type and inserts casts,
//! so a `nat` variable where `int` is expected becomes an upcast.

use crate::{
    diagnostic::Report,
    lang::{
        common::prim,
        common::{
            Id,
            ds::map::{ArityMismatch, IdMap},
            notation::mixfix::Mixfix,
            source::{Phrase, Span},
        },
        el::ast as el,
        hints::input,
        il::{ast as il, fresh as il_fresh, var as il_var},
        traits::{free::FreeIds, print::Print},
    },
    note_phrase, phrase,
    runtime::{
        ops::typ::{
            Theta, TypeArityMismatch, TypeErrorKind, equiv_func_typ, equiv_typ, expand_typ,
            optimize_sub_typ, sub_typ, subst_not_typ, subst_params, subst_typ, subst_typs,
        },
        typdef::TypeDef,
    },
};

use super::{
    ElabErrorKind, EntityKind, TypeShape,
    attempt::{Attempt, choose_sequential, fail, fail_silent, finish},
    context::Context,
    dimension,
    error::{self, MigrationError as ElabError},
};

// == Checks

// - Identifiers

/// Checks that an identifier carries no suffix, as a type identifier must.
fn valid_tid(id: &Id) -> bool {
    id.strip_suffix().node == id.node
}

/// Rejects duplicated type parameters.
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

/// Finds the first repeated parameter while retaining the earlier occurrence.
fn find_repeated_tparam(tparams: &[el::TParam]) -> Option<(&Id, &Span)> {
    let mut seen = IdMap::new();
    for tparam in tparams {
        // Stop at the first duplicate in source order
        if let Some(span_previous) = seen.get(tparam) {
            return Some((tparam, *span_previous));
        }
        seen.insert(tparam.clone(), &tparam.span);
    }
    None
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

/// Requires the expanded type to be `text`.
fn as_text_typ(ctx: &Context, typ_il: &il::Typ) -> Attempt<()> {
    let typ_il = expand_typ(&ctx.tdenv, typ_il)?;
    match &typ_il.node {
        il::TypKind::Text => Ok(()),
        _ => fail(destruct_error(TypeShape::Text, typ_il.span.clone())),
    }
}

/// Destructs the expanded type into its element type and iteration.
fn as_iter_typ(ctx: &Context, typ_il: &il::Typ) -> Attempt<(il::Typ, il::Iter)> {
    let typ_il = expand_typ(&ctx.tdenv, typ_il)?.into_owned();
    let span = typ_il.span;
    let il::TypKind::Iter(typ_inner_il, iter_il) = typ_il.node else {
        return fail(destruct_error(TypeShape::Iteration, span));
    };
    Ok((*typ_inner_il, iter_il))
}

/// Destructs the expanded type into its tuple component types.
fn as_tuple_typ(ctx: &Context, typ_il: &il::Typ) -> Attempt<Vec<il::Typ>> {
    let typ_il = expand_typ(&ctx.tdenv, typ_il)?.into_owned();
    let span = typ_il.span;
    let il::TypKind::Tuple(typs_il) = typ_il.node else {
        return fail(destruct_error(TypeShape::Tuple, span));
    };
    Ok(typs_il)
}

/// Destructs the expanded type into the element type of a list.
fn as_list_typ(ctx: &Context, typ_il: &il::Typ) -> Attempt<il::Typ> {
    let typ_il = expand_typ(&ctx.tdenv, typ_il)?.into_owned();
    let span = typ_il.span;
    let il::TypKind::Iter(typ_inner_il, il::Iter::List) = typ_il.node else {
        return fail(destruct_error(TypeShape::List, span));
    };
    Ok(*typ_inner_il)
}

/// Destructs the expanded type into the fields of the struct type it names.
fn as_struct_typ(ctx: &Context, typ_il: &il::Typ) -> Attempt<Vec<il::TypField>> {
    let typ_il = expand_typ(&ctx.tdenv, typ_il)?;
    // Struct types are always named types with a definition
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

/// Builds a type-argument arity error.
fn arity_error(expected: usize, actual: usize, span: Span) -> ElabError {
    let mismatch = ArityMismatch::new(expected, actual);
    let mismatch = TypeArityMismatch::TypeArgument(mismatch);
    let type_error = TypeErrorKind::ArityMismatch(mismatch);
    ElabError::new(ElabErrorKind::ArityMismatch, span, type_error.to_string())
}

/// Elaborates a plain type, checking the type-argument arity of named types.
fn elab_plain_typ(ctx: &Context, plain_typ: &el::PlainTyp) -> Result<il::Typ, ElabError> {
    let typ_kind_il = match &plain_typ.node {
        // Primitive types map directly
        el::PlainTypKind::Bool => il::TypKind::Bool,
        el::PlainTypKind::Num(num_typ) => il::TypKind::Num(*num_typ),
        el::PlainTypKind::Text => il::TypKind::Text,
        // Named types must be defined and fully applied
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
        // Parentheses leave no trace in IL
        el::PlainTypKind::Paren(plain_typ) => {
            let typ_il = elab_plain_typ(ctx, plain_typ)?;
            typ_il.node
        }
        // Tuples elaborate componentwise
        el::PlainTypKind::Tuple(plain_typs) => {
            let mut typs_il = Vec::with_capacity(plain_typs.len());
            for plain_typ in plain_typs {
                let typ_il = elab_plain_typ(ctx, plain_typ)?;
                typs_il.push(typ_il);
            }
            il::TypKind::Tuple(typs_il)
        }
        // Iterations elaborate the element type
        el::PlainTypKind::Iter(plain_typ, iter) => {
            let typ_il = elab_plain_typ(ctx, plain_typ)?;
            il::TypKind::Iter(Box::new(typ_il), *iter)
        }
    };
    let typ_il = phrase!(node: typ_kind_il, span: plain_typ.span.clone());
    Ok(typ_il)
}

// - Notation types

/// Elaborates a plain or notation type into mixfix notation.
fn elab_not_typ(ctx: &Context, typ: &el::Typ) -> Result<il::NotTyp, ElabError> {
    match typ {
        // A plain type is a single notation argument
        el::Typ::Plain(plain_typ) => {
            let typ_il = elab_plain_typ(ctx, plain_typ)?;
            let not_typ_il = Mixfix::Arg(typ_il);
            let not_typ_il = phrase!(node: not_typ_il, span: plain_typ.span.clone());
            Ok(not_typ_il)
        }
        // Notation types mirror the mixfix shape
        el::Typ::Notation(not_typ) => {
            let not_typ_kind_il = match &not_typ.node {
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
                    let not_typ_l_il = elab_not_typ(ctx, typ_l)?;
                    let not_typ_r_il = elab_not_typ(ctx, typ_r)?;
                    Mixfix::Infix(
                        Box::new(not_typ_l_il.node),
                        atom.clone(),
                        Box::new(not_typ_r_il.node),
                    )
                }
                el::NotTypKind::Brack(atom_l, typ, atom_r) => {
                    let not_typ_il = elab_not_typ(ctx, typ)?;
                    Mixfix::Brack(atom_l.clone(), Box::new(not_typ_il.node), atom_r.clone())
                }
            };
            let not_typ_il = phrase!(node: not_typ_kind_il, span: not_typ.span.clone());
            Ok(not_typ_il)
        }
    }
}

// - Definition types

/// Expands a plain type used as a variant case into the cases it names.
///
/// With `syntax u = A | B`,
/// the case `u` in `syntax t = u | C` contributes `A` and `B`,
/// with the type arguments of `u` substituted.
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
        // Only a completed variant type can be extended
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
            // Substitute the type arguments into each inherited case
            let theta = Theta::from_lists(tparams, targs_il).map_err(|mismatch| {
                arity_error(mismatch.expected, mismatch.actual, typ_il.span.clone())
            })?;
            typ_cases_il
                .iter()
                .map(|il::TypCase { not_typ: not_typ_il, typ_origin: typ_origin_il, hints }| {
                    let find_subst = |id: &il::Id| theta.get(id);
                    let not_typ_il =
                        subst_not_typ(&find_subst, not_typ_il).map_err(ElabError::from)?;
                    let targs_il =
                        subst_typs(&find_subst, &typ_origin_il.node.targs).map_err(ElabError::from)?;
                    let typ_origin_il = phrase! {
                        node: il::TypOriginKind { id: typ_origin_il.node.id.clone(), targs: targs_il },
                        span: typ_origin_il.span.clone(),
                    };
                    Ok(il::TypCase { not_typ: not_typ_il, typ_origin: typ_origin_il, hints: hints.clone() })
                })
                .collect()
        }
        // Parameter and extern types have no cases to inherit
        TypeDef::Parameter | TypeDef::Extern => Err(ElabError::new(
            ElabErrorKind::InvalidTypeExtension,
            typ_il.span.clone(),
            "cannot extend a non-variant type",
        )),
    }
}

/// Elaborates the body of a type definition and builds its stored `TypeDef`.
fn elab_def_typ(
    ctx: &Context,
    id: &Id,
    tparams: &[el::TParam],
    def_typ: &el::DefTyp,
) -> Result<(TypeDef, il::DefTyp), ElabError> {
    let def_typ_il = match &def_typ.node {
        el::DefTypKind::Plain(plain_typ) => {
            let typ_il = elab_plain_typ(ctx, plain_typ)?;
            let def_typ_kind_il = il::DefTypKind::Plain(typ_il);
            phrase!(node: def_typ_kind_il, span: plain_typ.span.clone())
        }
        el::DefTypKind::Struct(fields) => {
            let mut typ_fields_il = Vec::with_capacity(fields.len());
            for el::TypField { atom, typ: plain_typ, .. } in fields {
                let typ_il = elab_plain_typ(ctx, plain_typ)?;
                typ_fields_il.push(il::TypField { atom: atom.clone(), typ: typ_il });
            }
            let def_typ_kind_il = il::DefTypKind::Struct(typ_fields_il);
            phrase!(node: def_typ_kind_il, span: def_typ.span.clone())
        }
        // Cases originate from this type applied to its own parameters
        el::DefTypKind::Variant(cases) => {
            let targs_il = tparams
                .iter()
                .map(|tparam| {
                    let typ_kind_il = il::TypKind::Var(tparam.clone(), vec![]);
                    phrase!(node: typ_kind_il, span: tparam.span.clone())
                })
                .collect();
            let typ_origin_kind_il = il::TypOriginKind { id: id.clone(), targs: targs_il };
            let typ_origin_il = phrase!(node: typ_origin_kind_il, span: id.span.clone());
            let mut typ_cases_il = vec![];
            // Plain cases inherit another variant, notation cases are new
            for el::TypCase { typ, hints } in cases {
                match typ {
                    el::Typ::Plain(plain_typ) => {
                        let typ_il = elab_plain_typ(ctx, plain_typ)?;
                        let typ_cases_plain_il = elab_typ_case_plain(ctx, &typ_il)?;
                        typ_cases_il.extend(typ_cases_plain_il);
                    }
                    el::Typ::Notation(_) => {
                        let not_typ_il = elab_not_typ(ctx, typ)?;
                        typ_cases_il.push(il::TypCase {
                            not_typ: not_typ_il,
                            typ_origin: typ_origin_il.clone(),
                            hints: hints.clone(),
                        });
                    }
                }
            }
            // Two cases with the same mixfix shape would be ambiguous
            for (index, typ_case_il) in typ_cases_il.iter().enumerate() {
                let mixop = typ_case_il.not_typ.node.to_mixop();
                if typ_cases_il[..index]
                    .iter()
                    .any(|typ_case_other_il| typ_case_other_il.not_typ.node.to_mixop() == mixop)
                {
                    return Err(ElabError::new(
                        ElabErrorKind::AmbiguousVariant,
                        def_typ.span.clone(),
                        "variant cases are ambiguous",
                    ));
                }
            }
            let def_typ_kind_il = il::DefTypKind::Variant(typ_cases_il);
            phrase!(node: def_typ_kind_il, span: def_typ.span.clone())
        }
    };
    let typdef = TypeDef::Defined(tparams.to_vec(), Box::new(def_typ_il.clone()));
    Ok((typdef, def_typ_il))
}

// == Elaboration helpers

/// Fails an attempt with a freshly built located error.
fn fail_attempt<T>(kind: ElabErrorKind, span: Span, message: impl Into<String>) -> Attempt<T> {
    fail(ElabError::new(kind, span, message))
}

/// Wraps a type kind with the span of the term it describes.
fn typ_at(typ_kind_il: il::TypKind, span: &Span) -> il::Typ {
    phrase!(node: typ_kind_il, span: span.clone())
}

// == Expressions

// - Expression type inference

/// Fails inference of a construct whose type cannot be synthesized.
fn fail_infer<T>(span: &Span, construct: &str) -> Attempt<T> {
    fail_attempt(
        ElabErrorKind::CannotInfer,
        span.clone(),
        format!("cannot infer type of {construct}"),
    )
}

/// Synthesizes the type of an expression bottom-up.
///
/// Constructs such as `eps`, structs, and notation
/// only elaborate against an expected type and fail here.
fn infer_exp(ctx: &mut Context, exp: &el::Exp) -> Attempt<il::Exp> {
    match &exp.node {
        // Inferable constructs dispatch to their rule
        el::ExpKind::Bool(value) => infer_bool_exp(ctx, &exp.span, *value),
        el::ExpKind::Num(_, value) => infer_num_exp(ctx, &exp.span, value),
        el::ExpKind::Text(value) => infer_text_exp(ctx, &exp.span, value),
        el::ExpKind::Id(id) => infer_id_exp(ctx, &exp.span, id),
        el::ExpKind::Un(op, exp_inner) => infer_un_exp(ctx, &exp.span, *op, exp_inner),
        el::ExpKind::Bin(exp_l, op, exp_r) => infer_bin_exp(ctx, &exp.span, exp_l, *op, exp_r),
        el::ExpKind::Cmp(exp_l, op, exp_r) => infer_cmp_exp(ctx, &exp.span, exp_l, *op, exp_r),
        el::ExpKind::Arith(exp_inner) => infer_arith_exp(ctx, &exp.span, exp_inner),
        el::ExpKind::List(exps) => infer_list_exp(ctx, &exp.span, exps),
        el::ExpKind::Cons(exp_head, exp_tail) => infer_cons_exp(ctx, &exp.span, exp_head, exp_tail),
        el::ExpKind::Cat(exp_l, exp_r) => infer_cat_exp(ctx, &exp.span, exp_l, exp_r),
        el::ExpKind::Idx(exp_base, exp_idx) => infer_idx_exp(ctx, &exp.span, exp_base, exp_idx),
        el::ExpKind::Slice(exp_base, exp_idx, exp_len) => {
            infer_slice_exp(ctx, &exp.span, exp_base, exp_idx, exp_len)
        }
        el::ExpKind::Tuple(exps) => infer_tuple_exp(ctx, &exp.span, exps),
        el::ExpKind::Len(exp_inner) => infer_len_exp(ctx, &exp.span, exp_inner),
        el::ExpKind::Mem(exp_elem, exp_set) => infer_mem_exp(ctx, &exp.span, exp_elem, exp_set),
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
        // Constructs that need an expected type cannot be inferred
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
        note: il::TypKind::Num(prim::num::to_typ(value)),
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

// - Identifier expression inference

/// Looks up the type of a variable through its meta-variable.
fn infer_id_exp(ctx: &mut Context, span: &Span, id: &Id) -> Attempt<il::Exp> {
    // The suffix is dropped to find the governing meta-variable
    let tid = id.strip_suffix();
    let Some(typ_il) = ctx.find_metavar_opt(&tid) else {
        return fail_infer(&id.span, "variable");
    };
    let exp_il = crate::note_phrase!(node: crate::lang::il::ast::ExpKind::Id(id.clone()), note: typ_il.node.clone(), span: span.clone());
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

/// Infers a unary expression by trying each operand type the operator accepts.
fn infer_un_exp(ctx: &mut Context, span: &Span, op: el::UnOp, exp: &el::Exp) -> Attempt<il::Exp> {
    // Infer the type of the operand
    let exp_il = infer_exp(ctx, exp)?;
    // Candidates of (operator type, operand type, result type)
    let candidates_il = match op {
        el::UnOp::Bool(_) => vec![(il::OpTyp::Bool, il::TypKind::Bool, il::TypKind::Bool)],
        el::UnOp::Num(_) => vec![
            (
                il::OpTyp::Nat,
                il::TypKind::Num(prim::num::Typ::Nat),
                il::TypKind::Num(prim::num::Typ::Nat),
            ),
            (
                il::OpTyp::Int,
                il::TypKind::Num(prim::num::Typ::Int),
                il::TypKind::Num(prim::num::Typ::Int),
            ),
        ],
    };
    // Try elaboration for each candidate
    for (op_typ_il, typ_operand_il, typ_result_il) in candidates_il {
        // Check if the operand can be cast to the expected type
        let typ_expect_il = typ_at(typ_operand_il, &exp_il.span);
        if let Ok(exp_il) = cast_exp(ctx, &typ_expect_il, exp_il.clone()) {
            let exp_il = note_phrase! {
                node: il::ExpKind::Un(op, op_typ_il, Box::new(exp_il)),
                note: typ_result_il,
                span: span.clone(),
            };
            return Ok(exp_il);
        }
    }
    operator_error(span.clone())
}

// - Binary expression inference

/// Infers a binary expression by trying each accepted pair of operand types.
fn infer_bin_exp(
    ctx: &mut Context,
    span: &Span,
    exp_l: &el::Exp,
    op: el::BinOp,
    exp_r: &el::Exp,
) -> Attempt<il::Exp> {
    // Infer the types of both operands
    let exp_l_il = infer_exp(ctx, exp_l)?;
    let exp_r_il = infer_exp(ctx, exp_r)?;
    // Candidates of (operator type, left type, right type, result type)
    let candidates_il = match op {
        el::BinOp::Bool(_) => {
            vec![(il::OpTyp::Bool, il::TypKind::Bool, il::TypKind::Bool, il::TypKind::Bool)]
        }
        // Subtraction on naturals yields an integer
        el::BinOp::Num(prim::num::BinOp::Sub) => vec![
            (
                il::OpTyp::Int,
                il::TypKind::Num(prim::num::Typ::Nat),
                il::TypKind::Num(prim::num::Typ::Nat),
                il::TypKind::Num(prim::num::Typ::Int),
            ),
            (
                il::OpTyp::Int,
                il::TypKind::Num(prim::num::Typ::Int),
                il::TypKind::Num(prim::num::Typ::Int),
                il::TypKind::Num(prim::num::Typ::Int),
            ),
        ],
        el::BinOp::Num(_) => vec![
            (
                il::OpTyp::Nat,
                il::TypKind::Num(prim::num::Typ::Nat),
                il::TypKind::Num(prim::num::Typ::Nat),
                il::TypKind::Num(prim::num::Typ::Nat),
            ),
            (
                il::OpTyp::Int,
                il::TypKind::Num(prim::num::Typ::Int),
                il::TypKind::Num(prim::num::Typ::Int),
                il::TypKind::Num(prim::num::Typ::Int),
            ),
        ],
    };
    // Try each candidate, casting both operands to its operand types
    for (op_typ_il, typ_expect_l_il, typ_expect_r_il, typ_result_il) in candidates_il {
        let typ_expect_l_il = typ_at(typ_expect_l_il, &exp_l_il.span);
        let typ_expect_r_il = typ_at(typ_expect_r_il, &exp_r_il.span);
        let Ok(exp_l_il) = cast_exp(ctx, &typ_expect_l_il, exp_l_il.clone()) else {
            continue;
        };
        let Ok(exp_r_il) = cast_exp(ctx, &typ_expect_r_il, exp_r_il.clone()) else {
            continue;
        };
        let exp_il = note_phrase! {
            node: il::ExpKind::Bin(op, op_typ_il, Box::new(exp_l_il), Box::new(exp_r_il)),
            note: typ_result_il,
            span: span.clone(),
        };
        return Ok(exp_il);
    }
    operator_error(span.clone())
}

// - Comparison expression inference

/// Infers a comparison.
///
/// Equality checks one side against the other;
/// ordering tries each numeric type for both sides.
fn infer_cmp_exp(
    ctx: &mut Context,
    span: &Span,
    exp_l: &el::Exp,
    op: el::CmpOp,
    exp_r: &el::Exp,
) -> Attempt<il::Exp> {
    match op {
        // Equality: infer one side and check the other against it
        el::CmpOp::Bool(_) => choose_sequential(
            ctx,
            // Infer the right side, check the left against it
            |ctx| {
                let exp_r_il = infer_exp(ctx, exp_r)?;
                let typ_expect_l_il =
                    phrase!(node: exp_r_il.note.as_ref().clone(), span: exp_r_il.span.clone());
                let exp_l_il = elab_exp(ctx, &typ_expect_l_il, exp_l)?;
                let exp_il = note_phrase! {
                    node: il::ExpKind::Cmp(op, il::OpTyp::Bool, Box::new(exp_l_il), Box::new(exp_r_il)),
                    note: il::TypKind::Bool,
                    span: span.clone(),
                };
                Ok(exp_il)
            },
            // Infer the left side, check the right against it
            |ctx| {
                let exp_l_il = infer_exp(ctx, exp_l)?;
                let typ_expect_r_il =
                    phrase!(node: exp_l_il.note.as_ref().clone(), span: exp_l_il.span.clone());
                let exp_r_il = elab_exp(ctx, &typ_expect_r_il, exp_r)?;
                let exp_il = note_phrase! {
                    node: il::ExpKind::Cmp(op, il::OpTyp::Bool, Box::new(exp_l_il), Box::new(exp_r_il)),
                    note: il::TypKind::Bool,
                    span: span.clone(),
                };
                Ok(exp_il)
            },
        ),
        // Ordering: both sides must cast to the same numeric type
        el::CmpOp::Num(_) => {
            let exp_l_il = infer_exp(ctx, exp_l)?;
            let exp_r_il = infer_exp(ctx, exp_r)?;
            for (op_typ_il, typ_expect_kind_il) in [
                (il::OpTyp::Nat, il::TypKind::Num(prim::num::Typ::Nat)),
                (il::OpTyp::Int, il::TypKind::Num(prim::num::Typ::Int)),
            ] {
                let typ_expect_l_il = typ_at(typ_expect_kind_il.clone(), &exp_l_il.span);
                let typ_expect_r_il = typ_at(typ_expect_kind_il, &exp_r_il.span);
                let Ok(exp_l_il) = cast_exp(ctx, &typ_expect_l_il, exp_l_il.clone()) else {
                    continue;
                };
                let Ok(exp_r_il) = cast_exp(ctx, &typ_expect_r_il, exp_r_il.clone()) else {
                    continue;
                };
                let exp_il = note_phrase! {
                    node: il::ExpKind::Cmp(op, op_typ_il, Box::new(exp_l_il), Box::new(exp_r_il)),
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

/// Infers the inner expression and widens its span to the arithmetic brackets.
fn infer_arith_exp(ctx: &mut Context, span: &Span, exp: &el::Exp) -> Attempt<il::Exp> {
    let mut exp_il = infer_exp(ctx, exp)?;
    exp_il.span = span.clone();
    Ok(exp_il)
}

// - List expression inference

/// Infers a non-empty list from its first element; the rest must agree.
fn infer_list_exp(ctx: &mut Context, span: &Span, exps: &[el::Exp]) -> Attempt<il::Exp> {
    let Some((exp_first, exps_rest)) = exps.split_first() else {
        return fail_infer(span, "empty list");
    };
    // The element type comes from the first element
    let exp_first_il = infer_exp(ctx, exp_first)?;
    let typ_first_il =
        phrase!(node: exp_first_il.note.as_ref().clone(), span: exp_first_il.span.clone());
    let mut exps_rest_il = infer_exps(ctx, exps_rest)?;
    // Remaining elements must have an equivalent type
    for exp_il in &exps_rest_il {
        let typ_il = phrase!(node: exp_il.note.as_ref().clone(), span: exp_il.span.clone());
        let equivalent = equiv_typ(&ctx.tdenv, &typ_first_il, &typ_il)?;
        if !equivalent {
            return fail_infer(span, "list with heterogeneous elements");
        }
    }
    let mut exps_il = vec![exp_first_il];
    exps_il.append(&mut exps_rest_il);
    let exp_il = note_phrase! {
        node: il::ExpKind::List(exps_il),
        note: il::TypKind::Iter(Box::new(typ_first_il), il::Iter::List),
        span: span.clone(),
    };
    Ok(exp_il)
}

// - Cons expression inference

/// Infers a cons from its head and checks the tail against that list type.
fn infer_cons_exp(
    ctx: &mut Context,
    span: &Span,
    exp_head: &el::Exp,
    exp_tail: &el::Exp,
) -> Attempt<il::Exp> {
    let exp_head_il = infer_exp(ctx, exp_head)?;
    let typ_head_il =
        phrase!(node: exp_head_il.note.as_ref().clone(), span: exp_head_il.span.clone());
    // The tail must be a list of the head's type
    let typ_list_kind_il = il::TypKind::Iter(Box::new(typ_head_il), il::Iter::List);
    let typ_list_il = phrase!(node: typ_list_kind_il, span: exp_head_il.span.clone());
    let exp_tail_il = elab_exp(ctx, &typ_list_il, exp_tail)?;
    let exp_il = note_phrase! {
        node: il::ExpKind::Cons(Box::new(exp_head_il), Box::new(exp_tail_il)),
        note: typ_list_il.node,
        span: span.clone(),
    };
    Ok(exp_il)
}

// - Concatenation expression inference

/// Infers a concatenation as lists first, then as texts.
fn infer_cat_exp(
    ctx: &mut Context,
    span: &Span,
    exp_l: &el::Exp,
    exp_r: &el::Exp,
) -> Attempt<il::Exp> {
    choose_sequential(
        ctx,
        // Lists: the right side must match the list type of the left
        |ctx| {
            let exp_l_il = infer_exp(ctx, exp_l)?;
            let typ_l_il =
                phrase!(node: exp_l_il.note.as_ref().clone(), span: exp_l_il.span.clone());
            let typ_base_il = as_list_typ(ctx, &typ_l_il)?;
            let typ_list_kind_il = il::TypKind::Iter(Box::new(typ_base_il.clone()), il::Iter::List);
            let typ_list_il = phrase!(node: typ_list_kind_il, span: typ_base_il.span);
            let exp_r_il = elab_exp(ctx, &typ_list_il, exp_r)?;
            let exp_il = note_phrase! {
                node: il::ExpKind::Cat(Box::new(exp_l_il), Box::new(exp_r_il)),
                note: typ_list_il.node,
                span: span.clone(),
            };
            Ok(exp_il)
        },
        // Texts: both sides must elaborate as text
        |ctx| {
            let typ_text_l_il = typ_at(il::TypKind::Text, &exp_l.span);
            let exp_l_il = elab_exp(ctx, &typ_text_l_il, exp_l)?;
            let typ_text_r_il = typ_at(il::TypKind::Text, &exp_r.span);
            let exp_r_il = elab_exp(ctx, &typ_text_r_il, exp_r)?;
            let exp_il = note_phrase! {
                node: il::ExpKind::Cat(Box::new(exp_l_il), Box::new(exp_r_il)),
                note: il::TypKind::Text,
                span: span.clone(),
            };
            Ok(exp_il)
        },
    )
}

// - Tuple expression inference

/// Infers a tuple from the types of its components.
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

/// Infers a length of a list first, then of a text.
fn infer_len_exp(ctx: &mut Context, span: &Span, exp: &el::Exp) -> Attempt<il::Exp> {
    choose_sequential(
        ctx,
        // Length of a list
        |ctx| {
            let exp_il = infer_exp(ctx, exp)?;
            let typ_il = phrase!(node: exp_il.note.as_ref().clone(), span: exp_il.span.clone());
            as_list_typ(ctx, &typ_il)?;
            let exp_il = note_phrase! {
                node: il::ExpKind::Len(Box::new(exp_il)),
                note: il::TypKind::Num(prim::num::Typ::Nat),
                span: span.clone(),
            };
            Ok(exp_il)
        },
        // Length of a text
        |ctx| {
            let typ_text_il = typ_at(il::TypKind::Text, &exp.span);
            let exp_il = elab_exp(ctx, &typ_text_il, exp)?;
            let exp_il = note_phrase! {
                node: il::ExpKind::Len(Box::new(exp_il)),
                note: il::TypKind::Num(prim::num::Typ::Nat),
                span: span.clone(),
            };
            Ok(exp_il)
        },
    )
}

// - Membership expression inference

/// Infers a membership test from the element first, then from the list.
fn infer_mem_exp(
    ctx: &mut Context,
    span: &Span,
    exp_elem: &el::Exp,
    exp_set: &el::Exp,
) -> Attempt<il::Exp> {
    choose_sequential(
        ctx,
        // Element first: the list must hold elements of its type
        |ctx| {
            let exp_elem_il = infer_exp(ctx, exp_elem)?;
            let typ_elem_il = phrase! {
                node: exp_elem_il.note.as_ref().clone(),
                span: exp_elem_il.span.clone(),
            };
            let typ_list_kind_il = il::TypKind::Iter(Box::new(typ_elem_il), il::Iter::List);
            let typ_list_il = phrase!(node: typ_list_kind_il, span: exp_set.span.clone());
            let exp_set_il = elab_exp(ctx, &typ_list_il, exp_set)?;
            let exp_il = note_phrase! {
                node: il::ExpKind::Mem(Box::new(exp_elem_il), Box::new(exp_set_il)),
                note: il::TypKind::Bool,
                span: span.clone(),
            };
            Ok(exp_il)
        },
        // List first: the element must have its element type
        |ctx| {
            let exp_set_il = infer_exp(ctx, exp_set)?;
            let typ_set_il =
                phrase!(node: exp_set_il.note.as_ref().clone(), span: exp_set_il.span.clone());
            let typ_elem_il = as_list_typ(ctx, &typ_set_il)?;
            let exp_elem_il = elab_exp(ctx, &typ_elem_il, exp_elem)?;
            let exp_il = note_phrase! {
                node: il::ExpKind::Mem(Box::new(exp_elem_il), Box::new(exp_set_il)),
                note: il::TypKind::Bool,
                span: span.clone(),
            };
            Ok(exp_il)
        },
    )
}

// - Index expression inference

/// Infers an index into a list first, then into a text.
fn infer_idx_exp(
    ctx: &mut Context,
    span: &Span,
    exp_base: &el::Exp,
    exp_idx: &el::Exp,
) -> Attempt<il::Exp> {
    choose_sequential(
        ctx,
        // Index into a list: the element type is the result
        |ctx| {
            let exp_base_il = infer_exp(ctx, exp_base)?;
            let typ_base_il =
                phrase!(node: exp_base_il.note.as_ref().clone(), span: exp_base_il.span.clone());
            let typ_elem_il = as_list_typ(ctx, &typ_base_il)?;
            let typ_nat_il = typ_at(il::TypKind::Num(prim::num::Typ::Nat), &exp_idx.span);
            let exp_idx_il = elab_exp(ctx, &typ_nat_il, exp_idx)?;
            let exp_il = note_phrase! {
                node: il::ExpKind::Idx(Box::new(exp_base_il), Box::new(exp_idx_il)),
                note: typ_elem_il.node,
                span: span.clone(),
            };
            Ok(exp_il)
        },
        // Index into a text yields a text
        |ctx| {
            let typ_text_il = typ_at(il::TypKind::Text, &exp_base.span);
            let exp_base_il = elab_exp(ctx, &typ_text_il, exp_base)?;
            let typ_nat_il = typ_at(il::TypKind::Num(prim::num::Typ::Nat), &exp_idx.span);
            let exp_idx_il = elab_exp(ctx, &typ_nat_il, exp_idx)?;
            let exp_il = note_phrase! {
                node: il::ExpKind::Idx(Box::new(exp_base_il), Box::new(exp_idx_il)),
                note: il::TypKind::Text,
                span: span.clone(),
            };
            Ok(exp_il)
        },
    )
}

// - Slice expression inference

/// Infers a slice of a list first, then of a text.
fn infer_slice_exp(
    ctx: &mut Context,
    span: &Span,
    exp_base: &el::Exp,
    exp_idx: &el::Exp,
    exp_len: &el::Exp,
) -> Attempt<il::Exp> {
    choose_sequential(
        ctx,
        // Slice of a list keeps the list type
        |ctx| {
            let exp_base_il = infer_exp(ctx, exp_base)?;
            let typ_base_il =
                phrase!(node: exp_base_il.note.as_ref().clone(), span: exp_base_il.span.clone());
            as_list_typ(ctx, &typ_base_il)?;
            let typ_nat_il = typ_at(il::TypKind::Num(prim::num::Typ::Nat), &exp_idx.span);
            let exp_idx_il = elab_exp(ctx, &typ_nat_il, exp_idx)?;
            let typ_nat_il = typ_at(il::TypKind::Num(prim::num::Typ::Nat), &exp_len.span);
            let exp_len_il = elab_exp(ctx, &typ_nat_il, exp_len)?;
            let exp_il = note_phrase! { node: il::ExpKind::Slice(
                Box::new(exp_base_il),
                Box::new(exp_idx_il),
                Box::new(exp_len_il),
            ), note: typ_base_il.node, span: span.clone() };
            Ok(exp_il)
        },
        // Slice of a text
        |ctx| {
            let typ_text_il = typ_at(il::TypKind::Text, &exp_base.span);
            let exp_base_il = elab_exp(ctx, &typ_text_il, exp_base)?;
            let typ_nat_il = typ_at(il::TypKind::Num(prim::num::Typ::Nat), &exp_idx.span);
            let exp_idx_il = elab_exp(ctx, &typ_nat_il, exp_idx)?;
            let typ_nat_il = typ_at(il::TypKind::Num(prim::num::Typ::Nat), &exp_len.span);
            let exp_len_il = elab_exp(ctx, &typ_nat_il, exp_len)?;
            let exp_il = note_phrase! { node: il::ExpKind::Slice(
                Box::new(exp_base_il),
                Box::new(exp_idx_il),
                Box::new(exp_len_il),
            ), note: il::TypKind::Text, span: span.clone() };
            Ok(exp_il)
        },
    )
}

// - Dot expression inference

/// Infers a field access by looking the field up in the struct type.
fn infer_dot_exp(
    ctx: &mut Context,
    span: &Span,
    exp: &el::Exp,
    atom: &el::Atom,
) -> Attempt<il::Exp> {
    let exp_il = infer_exp(ctx, exp)?;
    let typ_il = phrase!(node: exp_il.note.as_ref().clone(), span: exp_il.span.clone());
    // The field must exist in the struct type
    let typ_fields_il = as_struct_typ(ctx, &typ_il)?;
    let Some(il::TypField { typ: typ_field_il, .. }) = typ_fields_il
        .iter()
        .find(|il::TypField { atom: atom_field, .. }| atom_field.node == atom.node)
    else {
        return fail_infer(&atom.span, "field");
    };
    let exp_il = note_phrase! {
        node: il::ExpKind::Dot(Box::new(exp_il), atom.clone()),
        note: typ_field_il.node.clone(),
        span: span.clone(),
    };
    Ok(exp_il)
}

// - Update expression inference

/// Infers a field update, checking the new value against the type at the path.
fn infer_upd_exp(
    ctx: &mut Context,
    span: &Span,
    exp_base: &el::Exp,
    path: &el::Path,
    exp_field: &el::Exp,
) -> Attempt<il::Exp> {
    let exp_base_il = infer_exp(ctx, exp_base)?;
    let typ_base_il =
        phrase!(node: exp_base_il.note.as_ref().clone(), span: exp_base_il.span.clone());
    // The path from the base type gives the field type
    let path_il = elab_path(ctx, &typ_base_il, path)?;
    let typ_field_il = typ_at(path_il.note.as_ref().clone(), &path_il.span);
    let exp_field_il = elab_exp(ctx, &typ_field_il, exp_field)?;
    let exp_il = note_phrase! {
        node: il::ExpKind::Upd(Box::new(exp_base_il), Box::new(path_il), Box::new(exp_field_il)),
        note: typ_base_il.node,
        span: span.clone(),
    };
    Ok(exp_il)
}

// - Parenthesized expression inference

/// Infers the inner expression and widens its span to the parentheses.
fn infer_paren_exp(ctx: &mut Context, span: &Span, exp: &el::Exp) -> Attempt<il::Exp> {
    let mut exp_il = infer_exp(ctx, exp)?;
    exp_il.span = span.clone();
    Ok(exp_il)
}

// - Call expression inference

/// Infers a call by instantiating the signature with the type arguments.
fn infer_call_exp(
    ctx: &mut Context,
    span: &Span,
    id: &Id,
    targs: &[el::Targ],
    args: &[el::Arg],
) -> Attempt<il::Exp> {
    let (tparams_il, params_il, typ_ret_il) = match ctx.find_func_signature(id) {
        Ok((tparams, params, typ_ret)) => (tparams.to_vec(), params.to_vec(), typ_ret.clone()),
        Err(error) => return fail(error),
    };
    // Type arguments must match the declared type parameters
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
            return fail(arity_error(mismatch.expected, mismatch.actual, id.span.clone()));
        }
    };
    // Substitute the type arguments into the parameters and return type
    let find_subst = |id: &il::Id| theta.get(id);
    let params_il = subst_params(&find_subst, &params_il)?;
    let typ_ret_il = subst_typ(&find_subst, &typ_ret_il)?;
    // Check the arguments against the instantiated parameters
    let args_il = elab_args(ctx, &params_il, args, false, span)?;
    let exp_il = note_phrase! {
        node: il::ExpKind::Call(id.clone(), targs_il, args_il),
        note: typ_ret_il.node,
        span: span.clone(),
    };
    Ok(exp_il)
}

// - Iterated expression inference

/// Infers an iterated expression, leaving its variables to dimension analysis.
fn infer_iter_exp(
    ctx: &mut Context,
    span: &Span,
    exp: &el::Exp,
    iter: el::Iter,
) -> Attempt<il::Exp> {
    let exp_il = infer_exp(ctx, exp)?;
    let typ_il = phrase!(node: exp_il.note.as_ref().clone(), span: exp_il.span.clone());
    let iter_il = iter;
    let exp_il = note_phrase! {
        node: il::ExpKind::Iter(Box::new(exp_il), il::ExpIter { iter: iter_il, vars: vec![] }),
        note: il::TypKind::Iter(Box::new(typ_il), iter_il),
        span: span.clone(),
    };
    Ok(exp_il)
}

// - Subtype expression inference

/// Infers a subtype test, which requires the two types to be comparable.
fn infer_sub_exp(
    ctx: &mut Context,
    span: &Span,
    exp: &el::Exp,
    plain_typ: &el::PlainTyp,
) -> Attempt<il::Exp> {
    let exp_il = infer_exp(ctx, exp)?;
    let typ_source_il = phrase!(node: exp_il.note.as_ref().clone(), span: exp_il.span.clone());
    let typ_target_il = match elab_plain_typ(ctx, plain_typ) {
        Ok(typ_il) => typ_il,
        Err(error) => return fail(error),
    };
    // The types must be related in at least one direction
    let source_sub = sub_typ(&ctx.tdenv, &typ_source_il, &typ_target_il)?;
    let target_sub = sub_typ(&ctx.tdenv, &typ_target_il, &typ_source_il)?;
    if !source_sub && !target_sub {
        return fail_attempt(
            ElabErrorKind::TypeMismatch,
            exp_il.span.clone(),
            "subtype expression compares incomparable types",
        );
    }
    // Precompute the runtime check the test performs
    let check = optimize_sub_typ(&ctx.tdenv, &typ_source_il, &typ_target_il)?;
    let exp_il = note_phrase! {
        node: il::ExpKind::Sub(Box::new(exp_il), Box::new(typ_target_il), Box::new(check)),
        note: il::TypKind::Bool,
        span: span.clone(),
    };
    Ok(exp_il)
}

// - Expected-type expression elaboration

/// Accepts an inferred expression at the expected type, upcasting if needed.
fn cast_exp(ctx: &Context, typ_expect_il: &il::Typ, exp_il: il::Exp) -> Attempt<il::Exp> {
    let typ_infer_il = phrase!(node: exp_il.note.as_ref().clone(), span: exp_il.span.clone());
    // Equivalent types need no cast
    let equivalent = equiv_typ(&ctx.tdenv, typ_expect_il, &typ_infer_il)?;
    if equivalent {
        return Ok(exp_il);
    }
    // A subtype is upcast to the expected type
    let subtype = sub_typ(&ctx.tdenv, &typ_infer_il, typ_expect_il)?;
    if subtype {
        let span = exp_il.span.clone();
        let exp_il = note_phrase! {
            node: il::ExpKind::UpCast(Box::new(typ_expect_il.clone()), Box::new(exp_il)),
            note: typ_expect_il.node.clone(),
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

/// Moves the span of an expression and its casts to the enclosing parentheses.
fn respan_parenthesized_exp(exp_il: &mut il::Exp, span: &Span) {
    exp_il.span = span.clone();
    match &mut exp_il.node {
        il::ExpKind::UpCast(_, exp_inner_il) | il::ExpKind::DownCast(_, exp_inner_il) => {
            respan_parenthesized_exp(exp_inner_il, span);
        }
        _ => {}
    }
}

/// Elaborates an expression against an expected type.
///
/// Failures are nested under one error for the whole expression
/// so that traces stay readable.
fn elab_exp(ctx: &mut Context, typ_expect_il: &il::Typ, exp: &el::Exp) -> Attempt<il::Exp> {
    let error = ElabError::new(
        ElabErrorKind::NoMatchingAlternative,
        exp.span.clone(),
        "expression elaboration failed",
    );
    // A parenthesized result takes the span of the parentheses
    let parenthesized = matches!(exp.node, el::ExpKind::Paren(_));
    let span = exp.span.clone();
    elab_exp_inner(ctx, typ_expect_il, exp)
        .map(move |mut exp_il| {
            if parenthesized {
                respan_parenthesized_exp(&mut exp_il, &span);
            }
            exp_il
        })
        .map_err(|failure| failure.nest(error))
}

/// Tries the singleton reading first when an iteration type is expected.
fn elab_exp_inner(ctx: &mut Context, typ_expect_il: &il::Typ, exp: &el::Exp) -> Attempt<il::Exp> {
    if let Ok((typ_base_il, iter_expect_il)) = as_iter_typ(ctx, typ_expect_il) {
        return choose_sequential(
            ctx,
            |ctx| elab_singleton_iter_exp(ctx, typ_expect_il, &typ_base_il, iter_expect_il, exp),
            |ctx| elab_exp_normal(ctx, typ_expect_il, exp),
        );
    }
    elab_exp_normal(ctx, typ_expect_il, exp)
}

// - Singleton iteration expression elaboration

/// Elaborates a `t` expression as a singleton where `t*` or `t?` is expected.
fn elab_singleton_iter_exp(
    ctx: &mut Context,
    typ_expect_il: &il::Typ,
    typ_base_il: &il::Typ,
    iter_expect_il: il::Iter,
    exp: &el::Exp,
) -> Attempt<il::Exp> {
    // Wildcards and empty sequences are never singletons
    if matches!(&exp.node, el::ExpKind::Id(id) if id.node == "_")
        || matches!(&exp.node, el::ExpKind::Eps)
        || matches!(&exp.node, el::ExpKind::List(exps) if exps.is_empty())
    {
        return fail_silent();
    }
    let exp_inner_il = elab_exp(ctx, typ_base_il, exp)?;
    let exp_kind_il = match iter_expect_il {
        il::Iter::Opt => il::ExpKind::Opt(Some(Box::new(exp_inner_il))),
        il::Iter::List => il::ExpKind::List(vec![exp_inner_il]),
    };
    Ok(note_phrase! {
        node: exp_kind_il,
        note: typ_expect_il.node.clone(),
        span: exp.span.clone(),
    })
}

// - Normal expression elaboration

/// Elaborates by inference and cast, falling back to contextual elaboration.
///
/// When inference fails,
/// a wildcard becomes a fresh variable,
/// a named expected type is unfolded into its plain, struct, or variant body,
/// and other constructs elaborate against the expected type directly.
fn elab_exp_normal(ctx: &mut Context, typ_expect_il: &il::Typ, exp: &el::Exp) -> Attempt<il::Exp> {
    // Try inference first, keeping its context only on success
    let mut ctx_candidate = ctx.clone();
    match infer_exp(&mut ctx_candidate, exp) {
        Ok(exp_il) => match cast_exp(&ctx_candidate, typ_expect_il, exp_il) {
            Ok(exp_il) => {
                *ctx = ctx_candidate;
                Ok(exp_il)
            }
            Err(failure) => Err(failure),
        },
        Err(failure_infer) => {
            // Retain inference diagnostics if contextual elaboration also fails
            elab_exp_normal_fallback(ctx, typ_expect_il, exp)
                .map_err(|failure| failure_infer.merge(failure))
        }
    }
}

/// Elaborates an expression against the shape of its expected type.
fn elab_exp_normal_fallback(
    ctx: &mut Context,
    typ_expect_il: &il::Typ,
    exp: &el::Exp,
) -> Attempt<il::Exp> {
    // A wildcard `_` becomes a fresh variable of the expected type
    if matches!(&exp.node, el::ExpKind::Id(id) if id.node == "_") {
        return elab_wildcard_exp(ctx, typ_expect_il, exp);
    }
    // Unfold a named expected type into its definition
    if let il::TypKind::Var(id, targs_il) = &typ_expect_il.node
        && let Some(TypeDef::Defined(tparams, def_typ_il)) = ctx.find_typdef_opt(id)
    {
        let theta = match Theta::from_lists(tparams, targs_il) {
            Ok(theta) => theta,
            Err(mismatch) => {
                return fail(arity_error(
                    mismatch.expected,
                    mismatch.actual,
                    typ_expect_il.span.clone(),
                ));
            }
        };
        match &def_typ_il.node {
            // Alias: elaborate against the aliased type
            il::DefTypKind::Plain(typ_il) => {
                let typ_il = subst_typ(&|id| theta.get(id), typ_il)?;
                return elab_exp_normal(ctx, &typ_il, exp);
            }
            // Struct: match the fields
            il::DefTypKind::Struct(typ_fields_il) => {
                let mut typ_fields_subst_il = Vec::with_capacity(typ_fields_il.len());
                for il::TypField { atom, typ: typ_il } in typ_fields_il {
                    let typ_il = subst_typ(&|id| theta.get(id), typ_il)?;
                    typ_fields_subst_il.push(il::TypField { atom: atom.clone(), typ: typ_il });
                }
                return elab_struct_exp(ctx, typ_expect_il, &typ_fields_subst_il, exp);
            }
            // Variant: match exactly one case
            il::DefTypKind::Variant(typ_cases_il) => {
                let mut typ_cases_subst_il = Vec::with_capacity(typ_cases_il.len());
                for il::TypCase { not_typ: not_typ_il, typ_origin: typ_origin_il, hints } in
                    typ_cases_il
                {
                    let find_subst = |id: &il::Id| theta.get(id);
                    let not_typ_il = subst_not_typ(&find_subst, not_typ_il)?;
                    let targs_il = subst_typs(&find_subst, &typ_origin_il.node.targs)?;
                    let typ_origin_il = phrase! {
                        node: il::TypOriginKind { id: typ_origin_il.node.id.clone(), targs: targs_il },
                        span: typ_origin_il.span.clone(),
                    };
                    typ_cases_subst_il.push(il::TypCase {
                        not_typ: not_typ_il,
                        typ_origin: typ_origin_il,
                        hints: hints.clone(),
                    });
                }
                return elab_variant_exp(ctx, typ_expect_il, &typ_cases_subst_il, exp);
            }
        }
    }
    // Other constructs are shaped by the expected type alone
    elab_plain_exp(ctx, typ_expect_il, exp)
}

// - Wildcard expression elaboration

/// Replaces `_` with a fresh variable of the expected type, recorded as free.
fn elab_wildcard_exp(
    ctx: &mut Context,
    typ_expect_il: &il::Typ,
    exp: &el::Exp,
) -> Attempt<il::Exp> {
    let var_il =
        il_fresh::var_from_typ_wildcard(&ctx.menv, &ctx.frees, exp.span.clone(), typ_expect_il);
    let exp_il = il_var::as_exp(false, &var_il);
    ctx.add_free(var_il.id);
    Ok(exp_il)
}

// - Plain expression elaboration

/// Elaborates constructs whose shape is fixed by the expected type.
fn elab_plain_exp(ctx: &mut Context, typ_expect_il: &il::Typ, exp: &el::Exp) -> Attempt<il::Exp> {
    let exp_kind_il = match &exp.node {
        el::ExpKind::Eps => elab_eps_exp(ctx, typ_expect_il)?,
        el::ExpKind::List(exps) => elab_list_exp(ctx, typ_expect_il, exps)?,
        el::ExpKind::Cons(exp_head, exp_tail) => {
            elab_cons_exp(ctx, typ_expect_il, exp_head, exp_tail)?
        }
        el::ExpKind::Cat(exp_l, exp_r) => elab_cat_exp(ctx, typ_expect_il, exp_l, exp_r)?,
        el::ExpKind::Tuple(exps) => elab_tuple_exp(ctx, typ_expect_il, exps)?,
        el::ExpKind::Paren(exp_inner) => elab_paren_exp(ctx, typ_expect_il, exp_inner)?,
        el::ExpKind::Iter(exp_inner, iter) => elab_iter_exp(ctx, typ_expect_il, exp_inner, *iter)?,
        // Anything else needed inference
        _ => {
            return fail_attempt(
                ElabErrorKind::NoMatchingAlternative,
                exp.span.clone(),
                "expression requires unsupported contextual elaboration",
            );
        }
    };
    Ok(note_phrase! {
        node: exp_kind_il,
        note: typ_expect_il.node.clone(),
        span: exp.span.clone(),
    })
}

// - Epsilon expression elaboration

/// Elaborates `eps` as the empty option or list the expected type requires.
fn elab_eps_exp(ctx: &Context, typ_expect_il: &il::Typ) -> Attempt<il::ExpKind> {
    let (_, iter_expect_il) = as_iter_typ(ctx, typ_expect_il)?;
    Ok(match iter_expect_il {
        il::Iter::Opt => il::ExpKind::Opt(None),
        il::Iter::List => il::ExpKind::List(vec![]),
    })
}

// - List expression elaboration

/// Elaborates a list literal element-wise against an expected list type.
fn elab_list_exp(
    ctx: &mut Context,
    typ_expect_il: &il::Typ,
    exps: &[el::Exp],
) -> Attempt<il::ExpKind> {
    let (typ_base_il, iter_expect_il) = as_iter_typ(ctx, typ_expect_il)?;
    // A list literal needs a list type, not an option
    if iter_expect_il != il::Iter::List {
        return fail_attempt(
            ElabErrorKind::InvalidIteration,
            typ_expect_il.span.clone(),
            "list expression has optional expected type",
        );
    }
    let mut exps_il = Vec::with_capacity(exps.len());
    for exp in exps {
        exps_il.push(elab_exp(ctx, &typ_base_il, exp)?);
    }
    Ok(il::ExpKind::List(exps_il))
}

// - Cons expression elaboration

/// Elaborates a cons against an expected iteration type.
fn elab_cons_exp(
    ctx: &mut Context,
    typ_expect_il: &il::Typ,
    exp_head: &el::Exp,
    exp_tail: &el::Exp,
) -> Attempt<il::ExpKind> {
    let (typ_base_il, iter_expect_il) = as_iter_typ(ctx, typ_expect_il)?;
    let exp_head_il = elab_exp(ctx, &typ_base_il, exp_head)?;
    let typ_tail_kind_il = il::TypKind::Iter(Box::new(typ_base_il), iter_expect_il);
    let typ_tail_il = phrase!(node: typ_tail_kind_il, span: typ_expect_il.span.clone());
    let exp_tail_il = elab_exp(ctx, &typ_tail_il, exp_tail)?;
    Ok(il::ExpKind::Cons(Box::new(exp_head_il), Box::new(exp_tail_il)))
}

// - Concatenation expression elaboration

/// Elaborates a concatenation as iterations first, then as texts.
fn elab_cat_exp(
    ctx: &mut Context,
    typ_expect_il: &il::Typ,
    exp_l: &el::Exp,
    exp_r: &el::Exp,
) -> Attempt<il::ExpKind> {
    choose_sequential(
        ctx,
        // Iterations: both sides at the expected type
        |ctx| {
            let (typ_base_il, iter_expect_il) = as_iter_typ(ctx, typ_expect_il)?;
            let typ_iter_kind_il = il::TypKind::Iter(Box::new(typ_base_il.clone()), iter_expect_il);
            let typ_iter_il = phrase!(node: typ_iter_kind_il, span: typ_base_il.span);
            let exp_l_il = elab_exp(ctx, &typ_iter_il, exp_l)?;
            let exp_r_il = elab_exp(ctx, &typ_iter_il, exp_r)?;
            Ok(il::ExpKind::Cat(Box::new(exp_l_il), Box::new(exp_r_il)))
        },
        // Texts
        |ctx| {
            let typ_text_il = typ_at(il::TypKind::Text, &exp_l.span);
            let exp_l_il = elab_exp(ctx, &typ_text_il, exp_l)?;
            let typ_text_il = typ_at(il::TypKind::Text, &exp_r.span);
            let exp_r_il = elab_exp(ctx, &typ_text_il, exp_r)?;
            Ok(il::ExpKind::Cat(Box::new(exp_l_il), Box::new(exp_r_il)))
        },
    )
}

// - Tuple expression elaboration

/// Elaborates a tuple component-wise against an expected tuple type.
fn elab_tuple_exp(
    ctx: &mut Context,
    typ_expect_il: &il::Typ,
    exps: &[el::Exp],
) -> Attempt<il::ExpKind> {
    // Component count must match the tuple type
    let typs_expect_il = as_tuple_typ(ctx, typ_expect_il)?;
    if typs_expect_il.len() != exps.len() {
        return fail_attempt(
            ElabErrorKind::ArityMismatch,
            typ_expect_il.span.clone(),
            "tuple expression arity does not match",
        );
    }
    let mut exps_il = Vec::with_capacity(exps.len());
    for (typ_expect_il, exp) in typs_expect_il.iter().zip(exps) {
        exps_il.push(elab_exp(ctx, typ_expect_il, exp)?);
    }
    Ok(il::ExpKind::Tuple(exps_il))
}

// - Parenthesized expression elaboration

fn elab_paren_exp(
    ctx: &mut Context,
    typ_expect_il: &il::Typ,
    exp: &el::Exp,
) -> Attempt<il::ExpKind> {
    let exp_il = elab_exp(ctx, typ_expect_il, exp)?;
    Ok(exp_il.node)
}

// - Iterated expression elaboration

/// Elaborates an iterated expression, requiring the expected iteration.
fn elab_iter_exp(
    ctx: &mut Context,
    typ_expect_il: &il::Typ,
    exp: &el::Exp,
    iter: el::Iter,
) -> Attempt<il::ExpKind> {
    let (typ_base_il, iter_expect_il) = as_iter_typ(ctx, typ_expect_il)?;
    // The iteration must match the expected one
    let iter_il = iter;
    if iter_il != iter_expect_il {
        return fail_attempt(
            ElabErrorKind::InvalidIteration,
            exp.span.clone(),
            "iteration mismatch",
        );
    }
    let exp_il = elab_exp(ctx, &typ_base_il, exp)?;
    Ok(il::ExpKind::Iter(Box::new(exp_il), il::ExpIter { iter: iter_il, vars: vec![] }))
}

// - Notation expression elaboration

/// Elaborates an expression against a notation type by matching its shape.
///
/// Against `C |- e : t`, the expression `C |- x : nat` matches atom by atom
/// while `C`, `x`, and `nat` elaborate against their argument types.
fn elab_not_exp(ctx: &mut Context, not_typ_il: &il::NotTyp, exp: &el::Exp) -> Attempt<il::NotExp> {
    // Parentheses around notation are transparent
    if let el::ExpKind::Paren(exp) = &exp.node {
        return elab_not_exp(ctx, not_typ_il, exp);
    }
    match (&not_typ_il.node, &exp.node) {
        // An argument elaborates against its type
        (Mixfix::Arg(typ_il), _) => {
            let exp_il = elab_exp(ctx, typ_il, exp)?;
            Ok(Mixfix::Arg(exp_il))
        }
        // Atoms must agree literally
        (Mixfix::Atom(atom_expect), el::ExpKind::Atom(atom)) if atom_expect.node == atom.node => {
            Ok(Mixfix::Atom(atom_expect.clone()))
        }
        // Sequences match element-wise
        (Mixfix::Seq(not_typs_il), el::ExpKind::Seq(exps)) => {
            if not_typs_il.len() != exps.len() {
                return fail_attempt(
                    ElabErrorKind::NoMatchingAlternative,
                    exp.span.clone(),
                    "notation sequence arity does not match",
                );
            }
            let mut not_exps_il = Vec::with_capacity(exps.len());
            for (not_typ_inner_il, exp) in not_typs_il.iter().zip(exps) {
                let not_typ_inner_il =
                    phrase!(node: not_typ_inner_il.clone(), span: not_typ_il.span.clone());
                let not_exp_il = elab_not_exp(ctx, &not_typ_inner_il, exp)?;
                not_exps_il.push(not_exp_il);
            }
            Ok(Mixfix::Seq(not_exps_il))
        }
        // Infix: the atom agrees, both sides match their notation
        (
            Mixfix::Infix(not_typ_l_il, atom_expect, not_typ_r_il),
            el::ExpKind::Infix(exp_l, atom, exp_r),
        ) if atom_expect.node == atom.node => {
            let not_typ_l_il =
                phrase!(node: (**not_typ_l_il).clone(), span: not_typ_il.span.clone());
            let not_typ_r_il =
                phrase!(node: (**not_typ_r_il).clone(), span: not_typ_il.span.clone());
            let not_exp_l_il = elab_not_exp(ctx, &not_typ_l_il, exp_l)?;
            let not_exp_r_il = elab_not_exp(ctx, &not_typ_r_il, exp_r)?;
            Ok(Mixfix::Infix(Box::new(not_exp_l_il), atom_expect.clone(), Box::new(not_exp_r_il)))
        }
        // Brackets: both atoms agree, the inside matches its notation
        (
            Mixfix::Brack(atom_expect_l, not_typ_inner_il, atom_expect_r),
            el::ExpKind::Brack(atom_l, exp_inner, atom_r),
        ) if atom_expect_l.node == atom_l.node && atom_expect_r.node == atom_r.node => {
            let not_typ_inner_il = phrase! {
                node: (**not_typ_inner_il).clone(),
                span: not_typ_il.span.clone(),
            };
            let not_exp_inner_il = elab_not_exp(ctx, &not_typ_inner_il, exp_inner)?;
            Ok(Mixfix::Brack(
                atom_expect_l.clone(),
                Box::new(not_exp_inner_il),
                atom_expect_r.clone(),
            ))
        }
        // Any other shape mismatch fails
        _ => fail_attempt(
            ElabErrorKind::NoMatchingAlternative,
            exp.span.clone(),
            "expression does not match notation",
        ),
    }
}

// - Struct expression elaboration

/// Elaborates a struct literal field by field in declaration order.
fn elab_struct_exp(
    ctx: &mut Context,
    typ_expect_il: &il::Typ,
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
    // Field count must match the struct type
    if typ_fields_il.len() != exp_fields.len() {
        return fail_attempt(
            ElabErrorKind::ArityMismatch,
            exp.span.clone(),
            "struct field count does not match",
        );
    }
    let mut exp_fields_il = Vec::with_capacity(exp_fields.len());
    for (il::TypField { atom: atom_expect, typ: typ_il }, (atom, exp_field)) in
        typ_fields_il.iter().zip(exp_fields)
    {
        // Fields must appear in declaration order
        if atom_expect.node != atom.node {
            return fail_attempt(
                ElabErrorKind::TypeMismatch,
                atom.span.clone(),
                "struct field does not match",
            );
        }
        let exp_field_il = elab_exp(ctx, typ_il, exp_field)?;
        exp_fields_il.push(il::ExpField { atom: atom_expect.clone(), exp: exp_field_il });
    }
    Ok(note_phrase! {
        node: il::ExpKind::Str(exp_fields_il),
        note: typ_expect_il.node.clone(),
        span: exp.span.clone(),
    })
}

// - Variant expression elaboration

/// Elaborates an expression against a variant type; one case must match.
fn elab_variant_exp(
    ctx: &mut Context,
    typ_expect_il: &il::Typ,
    typ_cases_il: &[il::TypCase],
    exp: &el::Exp,
) -> Attempt<il::Exp> {
    // Try each case on a copy of the context
    let mut ctx_match = ctx.clone();
    let mut exps_match_il = Vec::new();
    for il::TypCase { not_typ: not_typ_il, typ_origin: typ_origin_il, .. } in typ_cases_il {
        let mut ctx_candidate = ctx_match.clone();
        let not_exp_il = match elab_not_exp(&mut ctx_candidate, not_typ_il, exp) {
            Ok(not_exp_il) => not_exp_il,
            Err(_) => continue,
        };
        let typ_case_kind_il =
            il::TypKind::Var(typ_origin_il.node.id.clone(), typ_origin_il.node.targs.clone());
        let typ_case_il = phrase!(node: typ_case_kind_il, span: typ_origin_il.span.clone());
        let exp_case_kind_il = il::ExpKind::Case(Box::new(not_exp_il));
        let exp_case_il = note_phrase! {
            node: exp_case_kind_il,
            note: typ_case_il.node.clone(),
            span: exp.span.clone(),
        };
        // The case type must still cast to the expected type
        let exp_case_il = match cast_exp(&ctx_candidate, typ_expect_il, exp_case_il) {
            Ok(exp_case_il) => exp_case_il,
            Err(_) => continue,
        };
        ctx_match = ctx_candidate;
        exps_match_il.push(exp_case_il);
    }
    // Exactly one case may match
    match exps_match_il.len() {
        1 => {
            *ctx = ctx_match;
            Ok(exps_match_il.pop().expect("single variant match"))
        }
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

// == Paths

// - Path elaboration

fn elab_path(ctx: &mut Context, typ_expect_il: &il::Typ, path: &el::Path) -> Attempt<il::Path> {
    match &path.node {
        el::PathKind::Root => Ok(elab_root_path(&path.span, typ_expect_il)),
        el::PathKind::Idx(path_inner, exp_idx) => {
            elab_idx_path(ctx, &path.span, typ_expect_il, path_inner, exp_idx)
        }
        el::PathKind::Slice(path_inner, exp_idx, exp_len) => {
            elab_slice_path(ctx, &path.span, typ_expect_il, path_inner, exp_idx, exp_len)
        }
        el::PathKind::Dot(path_inner, atom) => {
            elab_dot_path(ctx, &path.span, typ_expect_il, path_inner, atom)
        }
    }
}

// - Root path elaboration

fn elab_root_path(span: &Span, typ_expect_il: &il::Typ) -> il::Path {
    note_phrase! {
        node: il::PathKind::Root,
        note: typ_expect_il.node.clone(),
        span: span.clone(),
    }
}

// - Index path elaboration

/// Elaborates an index path into a list first, then into a text.
fn elab_idx_path(
    ctx: &mut Context,
    span: &Span,
    typ_expect_il: &il::Typ,
    path_inner: &el::Path,
    exp_idx: &el::Exp,
) -> Attempt<il::Path> {
    choose_sequential(
        ctx,
        // Index into a list: the path continues at the element type
        |ctx| {
            let path_inner_il = elab_path(ctx, typ_expect_il, path_inner)?;
            let typ_inner_il = typ_at(path_inner_il.note.as_ref().clone(), &path_inner_il.span);
            let typ_elem_il = as_list_typ(ctx, &typ_inner_il)?;
            let typ_nat_il = typ_at(il::TypKind::Num(prim::num::Typ::Nat), &exp_idx.span);
            let exp_idx_il = elab_exp(ctx, &typ_nat_il, exp_idx)?;
            let path_kind_il = il::PathKind::Idx(Box::new(path_inner_il), Box::new(exp_idx_il));
            Ok(note_phrase! {
                node: path_kind_il,
                note: typ_elem_il.node,
                span: span.clone(),
            })
        },
        // Index into a text stays a text
        |ctx| {
            let path_inner_il = elab_path(ctx, typ_expect_il, path_inner)?;
            let typ_inner_il = typ_at(path_inner_il.note.as_ref().clone(), &path_inner_il.span);
            as_text_typ(ctx, &typ_inner_il)?;
            let typ_nat_il = typ_at(il::TypKind::Num(prim::num::Typ::Nat), &exp_idx.span);
            let exp_idx_il = elab_exp(ctx, &typ_nat_il, exp_idx)?;
            let path_kind_il = il::PathKind::Idx(Box::new(path_inner_il), Box::new(exp_idx_il));
            Ok(note_phrase! {
                node: path_kind_il,
                note: typ_inner_il.node,
                span: span.clone(),
            })
        },
    )
}

// - Slice path elaboration

/// Elaborates a slice path, which keeps the type of the list or text it slices.
fn elab_slice_path(
    ctx: &mut Context,
    span: &Span,
    typ_expect_il: &il::Typ,
    path_inner: &el::Path,
    exp_idx: &el::Exp,
    exp_len: &el::Exp,
) -> Attempt<il::Path> {
    let path_inner_il = elab_path(ctx, typ_expect_il, path_inner)?;
    let typ_inner_il = typ_at(path_inner_il.note.as_ref().clone(), &path_inner_il.span);
    // The path may slice a list or a text
    let is_list = as_list_typ(ctx, &typ_inner_il).is_ok();
    let is_text = as_text_typ(ctx, &typ_inner_il).is_ok();
    if !is_list && !is_text {
        return fail_attempt(
            ElabErrorKind::CannotDestructure(TypeShape::List),
            typ_inner_il.span,
            "slice path requires a list or text",
        );
    }
    let typ_nat_il = typ_at(il::TypKind::Num(prim::num::Typ::Nat), &exp_idx.span);
    let exp_idx_il = elab_exp(ctx, &typ_nat_il, exp_idx)?;
    let typ_nat_il = typ_at(il::TypKind::Num(prim::num::Typ::Nat), &exp_len.span);
    let exp_len_il = elab_exp(ctx, &typ_nat_il, exp_len)?;
    let path_kind_il =
        il::PathKind::Slice(Box::new(path_inner_il), Box::new(exp_idx_il), Box::new(exp_len_il));
    Ok(note_phrase! {
        node: path_kind_il,
        note: typ_inner_il.node,
        span: span.clone(),
    })
}

// - Dot path elaboration

/// Elaborates a field path by looking the field up in the struct type.
fn elab_dot_path(
    ctx: &mut Context,
    span: &Span,
    typ_expect_il: &il::Typ,
    path_inner: &el::Path,
    atom: &el::Atom,
) -> Attempt<il::Path> {
    let path_inner_il = elab_path(ctx, typ_expect_il, path_inner)?;
    let typ_inner_il = typ_at(path_inner_il.note.as_ref().clone(), &path_inner_il.span);
    // The field must exist in the struct type
    let typ_fields_il = as_struct_typ(ctx, &typ_inner_il)?;
    let Some(il::TypField { typ: typ_field_il, .. }) = typ_fields_il
        .into_iter()
        .find(|il::TypField { atom: atom_field, .. }| atom_field.node == atom.node)
    else {
        return fail_infer(&atom.span, "field");
    };
    let path_kind_il = il::PathKind::Dot(Box::new(path_inner_il), atom.clone());
    Ok(note_phrase! {
        node: path_kind_il,
        note: typ_field_il.node,
        span: span.clone(),
    })
}

// == Parameters and arguments

// - Parameter elaboration

/// Elaborates a parameter; a function parameter scopes its type parameters.
fn elab_param(ctx: &mut Context, param: &el::Param) -> Result<il::Param, ElabError> {
    let param_kind_il = match &param.node {
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
            // Type parameters scope over the parameters and return type only
            let (params_il, typ_ret_il) = {
                let mut ctx_local = ctx.clone();
                ctx_local.add_tparams(tparams)?;
                let params_il = params
                    .iter()
                    .map(|param| elab_param(&mut ctx_local, param))
                    .collect::<Result<Vec<_>, _>>()?;
                let typ_ret_il = elab_plain_typ(&ctx_local, plain_typ_ret)?;
                (params_il, typ_ret_il)
            };
            il::ParamKind::Def(id.clone(), tparams.clone(), params_il, typ_ret_il)
        }
    };
    let param_il = phrase!(node: param_kind_il, span: param.span.clone());
    Ok(param_il)
}

/// Gives the type a parameter binds: its plain type, or a function type.
fn typ_of_param(param_il: &il::Param) -> il::Typ {
    match &param_il.node {
        il::ParamKind::Exp(typ_il) => typ_il.clone(),
        // A function parameter has a function type
        il::ParamKind::Def(_, tparams_il, params_il, typ_ret_il) => {
            let func_typ_il = il::FuncTyp {
                tparams: tparams_il.clone(),
                typs_params: params_il.iter().map(typ_of_param).collect(),
                typ_ret: Box::new(typ_ret_il.clone()),
            };
            let typ_kind_il = il::TypKind::Func(func_typ_il);
            phrase!(node: typ_kind_il, span: param_il.span.clone())
        }
    }
}

// - Argument elaboration

/// Elaborates an argument against its parameter.
///
/// A function argument in a defining clause (`as_def`)
/// declares the parameter as a local function;
/// elsewhere it must name a function with an equivalent signature.
fn elab_arg(
    ctx: &mut Context,
    param_il: &il::Param,
    arg: &el::Arg,
    as_def: bool,
) -> Attempt<il::Arg> {
    match (&param_il.node, &arg.node) {
        // Expression arguments elaborate against the parameter type
        (il::ParamKind::Exp(typ_il), el::ArgKind::Exp(exp)) => {
            let exp_il = elab_exp(ctx, typ_il, exp)?;
            let arg_il = il::ArgKind::Exp(Box::new(exp_il));
            let arg_il = phrase!(node: arg_il, span: arg.span.clone());
            Ok(arg_il)
        }
        // Clause definition: bind the function parameter under its own name
        (
            il::ParamKind::Def(id_param, tparams_il, params_il, typ_ret_il),
            el::ArgKind::Def(id_arg),
        ) if as_def => {
            if id_param.node != id_arg.node {
                return fail_attempt(
                    ElabErrorKind::InvalidArgument,
                    arg.span.clone(),
                    "function argument does not match its declared parameter",
                );
            }
            let defined_func_il = il::DefinedFunc {
                id: id_param.clone(),
                tparams: tparams_il.clone(),
                params: params_il.clone(),
                typ: typ_ret_il.clone(),
                clauses: vec![],
                else_clause: None,
                hints: vec![],
            };
            if let Err(error) = ctx.add_defined_func(defined_func_il) {
                return fail(error);
            }
            let arg_il = il::ArgKind::Def(id_arg.clone());
            let arg_il = phrase!(node: arg_il, span: arg.span.clone());
            Ok(arg_il)
        }
        // Call: the named function must have an equivalent signature
        (il::ParamKind::Def(_, tparams_il, params_il, typ_ret_il), el::ArgKind::Def(id_arg)) => {
            let (tparams_arg_il, params_arg_il, typ_ret_arg_il) =
                match ctx.find_func_signature(id_arg) {
                    Ok(signature) => signature,
                    Err(error) => return fail(error),
                };
            let typ_param_il = il::FuncTyp {
                tparams: tparams_il.clone(),
                typs_params: params_il.iter().map(typ_of_param).collect(),
                typ_ret: Box::new(typ_ret_il.clone()),
            };
            let typ_arg_il = il::FuncTyp {
                tparams: tparams_arg_il.to_vec(),
                typs_params: params_arg_il.iter().map(typ_of_param).collect(),
                typ_ret: Box::new(typ_ret_arg_il.clone()),
            };
            let find_typdef_opt = |id: &il::Id| ctx.tdenv.get(id);
            let equivalent =
                equiv_func_typ(&find_typdef_opt, &arg.span, &typ_param_il, &typ_arg_il)?;
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

/// Elaborates arguments pairwise against parameters of matching count.
fn elab_args(
    ctx: &mut Context,
    params_il: &[il::Param],
    args: &[el::Arg],
    as_def: bool,
    span: &Span,
) -> Attempt<Vec<il::Arg>> {
    // Argument count must match parameter count
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

/// An elaborated premise, or a marker for premises that produce no IL premise.
#[allow(clippy::large_enum_variant)]
enum PremInternal {
    Some(il::Prem),
    /// A variable premise, which only extends the context.
    Var,
    /// An otherwise premise, which marks the fallback rule or clause.
    Else,
}

// - Premise elaboration

/// Elaborates a premise into an IL premise or a marker.
fn elab_prem(ctx: &mut Context, prem: &el::Prem) -> Attempt<PremInternal> {
    let prem_kind_il = match &prem.node {
        // Variable and otherwise premises leave no IL premise
        el::PremKind::Var(var_prem) => {
            elab_var_prem(ctx, var_prem)?;
            return Ok(PremInternal::Var);
        }
        el::PremKind::Rule(rule_prem) => elab_rule_prem(ctx, rule_prem)?,
        el::PremKind::RuleNot(rule_not_prem) => elab_rule_not_prem(ctx, rule_not_prem)?,
        el::PremKind::If(if_prem) => elab_if_prem(ctx, if_prem)?,
        // An otherwise premise marks the fallback
        el::PremKind::Else => return Ok(PremInternal::Else),
        el::PremKind::Iter(iter_prem) => elab_iter_prem(ctx, iter_prem)?,
        el::PremKind::Debug(debug_prem) => elab_debug_prem(ctx, debug_prem)?,
    };
    let prem_il = phrase!(node: prem_kind_il, span: prem.span.clone());
    Ok(PremInternal::Some(prem_il))
}

/// Elaborates premises in order and reports whether one was `otherwise`.
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
    // At most one otherwise premise
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

/// Binds the meta-variable declared by a variable premise.
fn elab_var_prem(ctx: &mut Context, prem: &el::VarPrem) -> Attempt<()> {
    if !valid_tid(&prem.id) {
        return fail_attempt(
            ElabErrorKind::InvalidIdentifier,
            prem.id.span.clone(),
            "invalid meta-variable identifier",
        );
    }
    // A meta-variable name must not clash with a type
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

/// Elaborates a rule premise; one without outputs becomes a holding check.
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
    // A premise without outputs only checks that the relation holds
    if conditional {
        Ok(il::PremKind::IfHold(il::IfHoldPrem { id: prem.id.clone(), not_exp: not_exp_il }))
    } else {
        Ok(il::PremKind::Rule(il::RulePrem {
            id: prem.id.clone(),
            not_exp: not_exp_il,
            input_hint,
        }))
    }
}

// - Negated rule premise elaboration

/// Elaborates a negated rule premise, which may not yield outputs.
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
    // A negated premise cannot bind outputs
    if !conditional {
        return fail_attempt(
            ElabErrorKind::InvalidPremise,
            prem.exp.span.clone(),
            "negated rule premise takes outputs",
        );
    }
    Ok(il::PremKind::IfNotHold(il::IfNotHoldPrem { id: prem.id.clone(), not_exp: not_exp_il }))
}

// - Conditional premise elaboration

fn elab_if_prem(ctx: &mut Context, prem: &el::IfPrem) -> Attempt<il::PremKind> {
    let typ_bool_il = typ_at(il::TypKind::Bool, &prem.exp.span);
    let exp_il = elab_exp(ctx, &typ_bool_il, &prem.exp)?;
    Ok(il::PremKind::If(il::IfPrem { exp: exp_il }))
}

// - Iteration premise elaboration

/// Elaborates an iterated premise, leaving its variables to dimension analysis.
fn elab_iter_prem(ctx: &mut Context, prem: &el::IterPrem) -> Attempt<il::PremKind> {
    let prem_inner_il = elab_prem(ctx, &prem.prem)?;
    // Only premises that produce an IL premise can be iterated
    let PremInternal::Some(prem_inner_il) = prem_inner_il else {
        return fail_attempt(
            ElabErrorKind::InvalidIteration,
            prem.prem.span.clone(),
            "cannot iterate variable or otherwise premise",
        );
    };
    let prem_iter_il = il::PremIter { iter: prem.iter, vars_bound: vec![], vars_bind: vec![] };
    Ok(il::PremKind::Iter(il::IterPrem { prem: Box::new(prem_inner_il), prem_iter: prem_iter_il }))
}

// - Debug premise elaboration

fn elab_debug_prem(ctx: &mut Context, prem: &el::DebugPrem) -> Attempt<il::PremKind> {
    let exp_il = infer_exp(ctx, &prem.exp)?;
    Ok(il::PremKind::Debug(il::DebugPrem { exp: exp_il }))
}

// == Rules and clauses

// - Rule elaboration

/// Elaborates one rule and reports whether it is the otherwise rule.
fn elab_rule(
    ctx: &mut Context,
    rule: &el::Rule,
    id_rel: &Id,
    not_typ_il: &il::NotTyp,
) -> Result<(il::Rule, bool), ElabError> {
    let el::RuleKind { id_rel: id_rel_rule, id_rule, exp, prems } = &rule.node;
    if id_rel_rule.node != id_rel.node {
        return Err(ElabError::new(
            ElabErrorKind::InvalidRule,
            id_rule.span.clone(),
            "rule relation does not match its group",
        ));
    }
    // Elaborate under a local context seeded with the rule's free identifiers
    let mut ctx_local = ctx.clone();
    ctx_local.reset_frees();
    let frees = rule.free_ids();
    ctx_local.add_frees(&frees);
    let not_exp_il = finish(elab_not_exp(&mut ctx_local, not_typ_il, exp))?;
    let (prems_il, is_else) = finish(elab_prems(&mut ctx_local, prems, &id_rule.span))?;
    let rule_kind_il = il::RuleKind { id: id_rule.clone(), not_exp: not_exp_il, prems: prems_il };
    let rule_il = phrase!(node: rule_kind_il, span: rule.span.clone());
    Ok((rule_il, is_else))
}

/// Elaborates a rule group into ordinary rules or a single otherwise rule.
fn elab_rule_group(
    ctx: &mut Context,
    def: &Phrase<&el::RuleGroupDef>,
) -> Result<(Option<il::RuleGroup>, Option<il::ElseGroup>), ElabError> {
    let span = &def.span;
    let def = def.node;
    let defined_rel_il = ctx.find_defined_rel(&def.relid)?;
    let not_typ_il = defined_rel_il.not_typ.clone();
    let mut rules_il = Vec::with_capacity(def.rules.len());
    let mut rules_else_il = Vec::new();
    // Sort the rules into ordinary and otherwise rules
    for rule in &def.rules {
        let (rule_il, is_else) = elab_rule(ctx, rule, &def.relid, &not_typ_il)?;
        if is_else {
            rules_else_il.push(rule_il);
        } else {
            rules_il.push(rule_il);
        }
    }
    // An otherwise rule must be alone in its group
    match rules_else_il.len() {
        0 => {
            let rule_group_kind_il = il::RuleGroupKind { id: def.groupid.clone(), rules: rules_il };
            let rule_group_il = phrase!(node: rule_group_kind_il, span: span.clone());
            Ok((Some(rule_group_il), None))
        }
        1 if def.rules.len() == 1 => {
            let else_group_kind_il =
                il::ElseGroupKind { id: def.groupid.clone(), rule: rules_else_il.remove(0) };
            let else_group_il = phrase!(node: else_group_kind_il, span: span.clone());
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

/// Elaborates one clause and reports whether it is the otherwise clause.
fn elab_clause(
    ctx: &mut Context,
    def: &Phrase<&el::FuncDef>,
) -> Result<(il::Clause, bool), ElabError> {
    let span = &def.span;
    let def = def.node;
    let il::DefinedFunc { tparams: tparams_expect_il, params: params_il, typ: typ_ret_il, .. } =
        ctx.find_defined_func(&def.id)?;
    // Type parameters must repeat the declaration exactly
    if def.tparams.len() != tparams_expect_il.len()
        || def
            .tparams
            .iter()
            .zip(tparams_expect_il)
            .any(|(tparam, tparam_expect_il)| tparam.node != tparam_expect_il.node)
    {
        return Err(ElabError::new(
            ElabErrorKind::ArityMismatch,
            def.id.span.clone(),
            "type parameters do not match",
        ));
    }
    let params_il = params_il.to_vec();
    let typ_ret_il = typ_ret_il.clone();
    // Local context with the clause's free identifiers and type parameters
    let mut ctx_local = ctx.clone();
    ctx_local.reset_frees();
    let frees = def.free_ids();
    ctx_local.add_frees(&frees);
    ctx_local.add_tparams(&def.tparams)?;
    let args_il = finish(elab_args(&mut ctx_local, &params_il, &def.args, true, span))?;
    let (prems_il, is_else) = finish(elab_prems(&mut ctx_local, &def.prems, span))?;
    let exp_il = finish(elab_exp(&mut ctx_local, &typ_ret_il, &def.exp))?;
    let clause_kind_il = il::ClauseKind { args: args_il, exp: exp_il, prems: prems_il };
    let clause_il = phrase!(node: clause_kind_il, span: span.clone());
    Ok((clause_il, is_else))
}

// == Definitions

// - Definition dispatch

/// Elaborates a definition; bodies for earlier declarations yield nothing.
fn elab_def(ctx: &mut Context, def_el: el::Def) -> Result<Option<il::Def>, ElabError> {
    let span = def_el.span;
    match def_el.node {
        // An extern type becomes an IL definition
        el::DefKind::ExternSyntax(extern_syntax_def) => {
            let def_kind_il = elab_extern_syntax_def(ctx, extern_syntax_def)?;
            let def_il = phrase!(node: def_kind_il, span: span);
            Ok(Some(def_il))
        }
        // Forward declarations only extend the context
        el::DefKind::Syntax(syntax_def) => {
            elab_syntax_def(ctx, &syntax_def)?;
            Ok(None)
        }
        // A type body completes its forward declaration
        el::DefKind::Typ(typ_def) => {
            let span = typ_def.def_typ.span.clone();
            let def_kind_il = elab_typ_def(ctx, typ_def)?;
            let def_il = phrase!(node: def_kind_il, span: span);
            Ok(Some(def_il))
        }
        // A meta-variable declaration
        el::DefKind::Var(var_def) => {
            let span = var_def.id.span.clone();
            let def_kind_il = elab_var_def(ctx, var_def)?;
            let def_il = phrase!(node: def_kind_il, span: span);
            Ok(Some(def_il))
        }
        // An extern relation declaration
        el::DefKind::ExternRel(extern_rel_def) => {
            let def_kind_il = elab_extern_rel_def(ctx, extern_rel_def, &span)?;
            let def_il = phrase!(node: def_kind_il, span: span);
            Ok(Some(def_il))
        }
        // A relation declaration; its rules arrive later
        el::DefKind::Rel(rel_def) => {
            let def_kind_il = elab_rel_def(ctx, rel_def, &span)?;
            let def_il = phrase!(node: def_kind_il, span: span);
            Ok(Some(def_il))
        }
        // Rule groups attach to their relation and yield no definition
        el::DefKind::RuleGroup(rule_group_def) => {
            let rule_group_def = crate::phrase! {
                node: &rule_group_def,
                span: span,
            };
            elab_rule_group_def(ctx, &rule_group_def)?;
            Ok(None)
        }
        // An extern function declaration
        el::DefKind::ExternDec(extern_dec_def) => {
            let def_kind_il = elab_extern_dec_def(ctx, extern_dec_def)?;
            let def_il = phrase!(node: def_kind_il, span: span);
            Ok(Some(def_il))
        }
        // A builtin function declaration
        el::DefKind::BuiltinDec(builtin_dec_def) => {
            let def_kind_il = elab_builtin_dec_def(ctx, builtin_dec_def)?;
            let def_il = phrase!(node: def_kind_il, span: span);
            Ok(Some(def_il))
        }
        // A table declaration; its rows arrive later
        el::DefKind::TableDec(table_dec_def) => {
            let def_kind_il = elab_table_dec_def(ctx, table_dec_def)?;
            let def_il = phrase!(node: def_kind_il, span: span);
            Ok(Some(def_il))
        }
        // A function declaration; its clauses arrive later
        el::DefKind::FuncDec(func_dec_def) => {
            let def_kind_il = elab_func_dec_def(ctx, func_dec_def)?;
            let def_il = phrase!(node: def_kind_il, span: span);
            Ok(Some(def_il))
        }
        // Table rows attach to their declared table
        el::DefKind::TableDef(table_def) => {
            elab_table_def(ctx, &table_def)?;
            Ok(None)
        }
        // A clause attaches to its declared function
        el::DefKind::FuncDef(func_def) => {
            let func_def = crate::phrase! {
                node: &func_def,
                span: span,
            };
            elab_func_def(ctx, &func_def)?;
            Ok(None)
        }
        // Separators carry nothing
        el::DefKind::Sep => Ok(None),
    }
}

// - Type declarations

/// Declares an extern type together with a meta-variable of that type.
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
    // An extern type also names a meta-variable
    ctx.add_typdef(def.id.clone(), TypeDef::Extern)?;
    let typ_kind_il = il::TypKind::Var(def.id.clone(), vec![]);
    let typ_il = phrase!(node: typ_kind_il, span: def.id.span.clone());
    ctx.add_metavar(def.id.clone(), typ_il)?;
    let extern_typ_il = il::ExternTyp { id: def.id, hints: def.hints };
    Ok(il::DefKind::Typ(il::TypDef::Extern(extern_typ_il)))
}

/// Forward-declares the types of a syntax block ahead of their bodies.
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
        // A type without parameters also names a meta-variable
        ctx.add_typdef(entry.id.clone(), TypeDef::Defining(entry.tparams.clone()))?;
        if entry.tparams.is_empty() {
            let typ_kind_il = il::TypKind::Var(entry.id.clone(), vec![]);
            let typ_il = phrase!(node: typ_kind_il, span: entry.id.span.clone());
            ctx.add_metavar(entry.id.clone(), typ_il)?;
        }
    }
    Ok(())
}

// - Type definitions

/// Elaborates a type body, completing a forward declaration or a new type.
fn elab_typ_def(ctx: &mut Context, def: el::TypDef) -> Result<il::DefKind, ElabError> {
    match ctx.find_typdef_opt(&def.id) {
        // A forward-declared type must repeat its parameters
        Some(TypeDef::Defining(tparams)) => {
            let matches = tparams.len() == def.tparams.len()
                && tparams
                    .iter()
                    .zip(&def.tparams)
                    .all(|(id_l, id_r)| id_l.node == id_r.node);
            if !matches {
                return Err(ElabError::new(
                    ElabErrorKind::ArityMismatch,
                    def.id.span.clone(),
                    "type parameters do not match",
                ));
            }
        }
        Some(TypeDef::Defined(_, _)) => {
            // The stored identifier still points at the first declaration
            let (id_previous, _) = ctx.tdenv.get_key_value(&def.id).expect("defined type");
            return Err(error::type_definition_repeated(&def.id, &id_previous.span).into());
        }
        Some(_) => {
            return Err(ElabError::new(
                ElabErrorKind::Duplicate(EntityKind::Type),
                def.id.span.clone(),
                "type was already defined",
            ));
        }
        // A new type is declared on the spot
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
                let typ_kind_il = il::TypKind::Var(def.id.clone(), vec![]);
                let typ_il = phrase!(node: typ_kind_il, span: def.id.span.clone());
                ctx.add_metavar(def.id.clone(), typ_il)?;
            }
        }
    }
    // Elaborate the body with the type parameters in scope
    let (typdef, def_typ_il) = {
        let mut ctx_local = ctx.clone();
        ctx_local.add_tparams(&def.tparams)?;
        elab_def_typ(&ctx_local, &def.id, &def.tparams, &def.def_typ)?
    };
    ctx.update_typdef(&def.id, typdef);
    let defined_typ_il =
        il::DefinedTyp { id: def.id, tparams: def.tparams, def_typ: def_typ_il, hints: def.hints };
    Ok(il::DefKind::Typ(il::TypDef::Defined(Box::new(defined_typ_il))))
}

// - Variable definitions

/// Declares a global meta-variable.
fn elab_var_def(ctx: &mut Context, def: el::VarDef) -> Result<il::DefKind, ElabError> {
    if !valid_tid(&def.id) {
        return Err(error::meta_variable_identifier_invalid(&def.id).into());
    }
    // A meta-variable name must not clash with a type
    if let Some((id_previous, _)) = ctx.tdenv.get_key_value(&def.id) {
        return Err(error::meta_variable_type_repeated(&def.id, &id_previous.span).into());
    }
    let typ_il = elab_plain_typ(ctx, &def.plain_typ)?;
    ctx.add_metavar(def.id.clone(), typ_il.clone())?;
    let var_def_il = il::VarDef { id: def.id, typ: typ_il, hints: def.hints };
    Ok(il::DefKind::Var(var_def_il))
}

// - Input hints

/// Reads a relation's `input` hint; by default every position is an input.
fn fetch_input_hint(
    span: &Span,
    not_typ_il: &il::NotTyp,
    hints: &[el::Hint],
) -> Result<input::InputHint, ElabError> {
    let arity = not_typ_il.node.arity();
    // Without a hint every position is an input
    let Some(el::Hint { exp: exp_hint, .. }) = hints.iter().find(|hint| hint.id.node == "input")
    else {
        return Ok(input::InputHint::new((0..arity).collect()));
    };
    let Some(input_hint) = input::init(exp_hint) else {
        return Err(ElabError::new(
            ElabErrorKind::InvalidInputHint,
            span.clone(),
            "malformed input hint",
        ));
    };
    // The hint must stay within the notation arity
    input::validate(&input_hint, arity).map_err(|error| {
        ElabError::new(ElabErrorKind::InvalidInputHint, span.clone(), error.to_string())
    })?;
    Ok(input_hint)
}

// - Relation definitions

/// Declares an extern relation.
fn elab_extern_rel_def(
    ctx: &mut Context,
    def: el::ExternRelDef,
    span: &Span,
) -> Result<il::DefKind, ElabError> {
    let typ = el::Typ::Notation(def.not_typ.clone());
    let not_typ_il = elab_not_typ(ctx, &typ)?;
    let input_hint = fetch_input_hint(span, &not_typ_il, &def.hints)?;
    let extern_rel_il =
        il::ExternRel { id: def.id, not_typ: not_typ_il, input_hint, hints: def.hints };
    ctx.add_extern_rel(extern_rel_il.clone())?;
    Ok(il::DefKind::Rel(il::RelDef::Extern(Box::new(extern_rel_il))))
}

/// Declares a relation whose rule groups arrive later.
fn elab_rel_def(ctx: &mut Context, def: el::RelDef, span: &Span) -> Result<il::DefKind, ElabError> {
    let typ = el::Typ::Notation(def.not_typ.clone());
    let not_typ_il = elab_not_typ(ctx, &typ)?;
    let input_hint = fetch_input_hint(span, &not_typ_il, &def.hints)?;
    let defined_rel_il = il::DefinedRel {
        id: def.id,
        not_typ: not_typ_il,
        input_hint,
        rule_groups: vec![],
        else_group: None,
        hints: def.hints,
    };
    ctx.add_defined_rel(defined_rel_il.clone())?;
    Ok(il::DefKind::Rel(il::RelDef::Defined(Box::new(defined_rel_il))))
}

// - Rule group definitions

/// Elaborates a rule group and attaches it to its relation.
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

/// Declares an extern function.
fn elab_extern_dec_def(ctx: &mut Context, def: el::ExternDecDef) -> Result<il::DefKind, ElabError> {
    // Label both occurrences of the first repeated type parameter
    if let Some((tparam, span_previous)) = find_repeated_tparam(&def.tparams) {
        return Err(error::function_extern_type_parameter_repeated(tparam, span_previous).into());
    }
    // Parameters and return type see the type parameters
    let (params_il, typ_il) = {
        let mut ctx_local = ctx.clone();
        ctx_local.add_tparams(&def.tparams)?;
        let params_il = def
            .params
            .iter()
            .map(|param| elab_param(&mut ctx_local, param))
            .collect::<Result<Vec<_>, _>>()?;
        let typ_il = elab_plain_typ(&ctx_local, &def.plain_typ)?;
        (params_il, typ_il)
    };
    let extern_func_il = il::ExternFunc {
        id: def.id,
        tparams: def.tparams,
        params: params_il,
        typ: typ_il,
        hints: def.hints,
    };
    ctx.add_extern_func(extern_func_il.clone())?;
    Ok(il::DefKind::MetaFunc(il::MetaFuncDef::Extern(extern_func_il)))
}

/// Declares a builtin function.
fn elab_builtin_dec_def(
    ctx: &mut Context,
    def: el::BuiltinDecDef,
) -> Result<il::DefKind, ElabError> {
    // Label both occurrences of the first repeated type parameter
    if let Some((tparam, span_previous)) = find_repeated_tparam(&def.tparams) {
        return Err(error::function_builtin_type_parameter_repeated(tparam, span_previous).into());
    }
    // Parameters and return type see the type parameters
    let (params_il, typ_il) = {
        let mut ctx_local = ctx.clone();
        ctx_local.add_tparams(&def.tparams)?;
        let params_il = def
            .params
            .iter()
            .map(|param| elab_param(&mut ctx_local, param))
            .collect::<Result<Vec<_>, _>>()?;
        let typ_il = elab_plain_typ(&ctx_local, &def.plain_typ)?;
        (params_il, typ_il)
    };
    let builtin_func_il = il::BuiltinFunc {
        id: def.id,
        tparams: def.tparams,
        params: params_il,
        typ: typ_il,
        hints: def.hints,
    };
    ctx.add_builtin_func(builtin_func_il.clone())?;
    Ok(il::DefKind::MetaFunc(il::MetaFuncDef::Builtin(builtin_func_il)))
}

/// Declares a table function, which takes plain parameters and returns `bool`.
fn elab_table_dec_def(ctx: &mut Context, def: el::TableDecDef) -> Result<il::DefKind, ElabError> {
    let params_il = def
        .params
        .iter()
        .map(|param| elab_param(ctx, param))
        .collect::<Result<Vec<_>, _>>()?;
    // Locate the offending function parameter rather than the whole declaration
    for param_il in &params_il {
        if let il::ParamKind::Def(id, _, _, _) = &param_il.node {
            return Err(error::table_parameter_unsupported(id, &param_il.span).into());
        }
    }
    // Accept boolean aliases under the same equivalence relation as other types
    let typ_il = elab_plain_typ(ctx, &def.plain_typ)?;
    let typ_bool_il = phrase!(node: il::TypKind::Bool, span: typ_il.span.clone());
    if !equiv_typ(&ctx.tdenv, &typ_il, &typ_bool_il)? {
        return Err(error::table_return_type_invalid(
            &def.id,
            &typ_il.span,
            &Print::to_string(&typ_il),
        )
        .into());
    }
    let table_func_il = il::TableFunc {
        id: def.id,
        params: params_il,
        typ: typ_il,
        rows: vec![],
        hints: def.hints,
    };
    ctx.add_table_func(table_func_il.clone())?;
    Ok(il::DefKind::MetaFunc(il::MetaFuncDef::Table(table_func_il)))
}

/// Declares a function whose clauses arrive later.
fn elab_func_dec_def(ctx: &mut Context, def: el::FuncDecDef) -> Result<il::DefKind, ElabError> {
    // Label both occurrences of the first repeated type parameter
    if let Some((tparam, span_previous)) = find_repeated_tparam(&def.tparams) {
        return Err(error::function_type_parameter_repeated(tparam, span_previous).into());
    }
    // Parameters and return type see the type parameters
    let (params_il, typ_il) = {
        let mut ctx_local = ctx.clone();
        ctx_local.add_tparams(&def.tparams)?;
        let params_il = def
            .params
            .iter()
            .map(|param| elab_param(&mut ctx_local, param))
            .collect::<Result<Vec<_>, _>>()?;
        let typ_il = elab_plain_typ(&ctx_local, &def.plain_typ)?;
        (params_il, typ_il)
    };
    let defined_func_il = il::DefinedFunc {
        id: def.id,
        tparams: def.tparams,
        params: params_il,
        typ: typ_il,
        clauses: vec![],
        else_clause: None,
        hints: def.hints,
    };
    ctx.add_defined_func(defined_func_il.clone())?;
    Ok(il::DefKind::MetaFunc(il::MetaFuncDef::Defined(Box::new(defined_func_il))))
}

// - Table function definitions

/// Elaborates table rows against the declared parameters and result type.
fn elab_table_def(ctx: &mut Context, def: &el::TableDef) -> Result<(), ElabError> {
    let table_func_il = ctx.find_table_func(&def.id)?;
    let params_il = table_func_il.params.clone();
    let typ_il = table_func_il.typ.clone();
    let mut rows_il = Vec::with_capacity(def.rows.len());
    for row in &def.rows {
        let el::TableRowKind { exp_pattern, exp_body } = &row.node;
        // A row pattern is a tuple of arguments or a single argument
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
        // Each row elaborates under its own free identifiers
        let (args_il, exp_body_il) = {
            let mut ctx_local = ctx.clone();
            ctx_local.reset_frees();
            let frees = row.free_ids();
            ctx_local.add_frees(&frees);
            let args_il = finish(elab_args(&mut ctx_local, &params_il, &args, true, &row.span))?;
            let exp_body_il = finish(elab_exp(&mut ctx_local, &typ_il, exp_body))?;
            (args_il, exp_body_il)
        };
        let row_il = phrase!(node: il::TableRowKind { args: args_il, exp: exp_body_il }, span: row.span.clone());
        rows_il.push(row_il);
    }
    ctx.add_table_func_rows(&def.id, rows_il)?;
    Ok(())
}

// - Function definitions

/// Elaborates a clause and attaches it to its declared function.
fn elab_func_def(ctx: &mut Context, def: &Phrase<&el::FuncDef>) -> Result<(), ElabError> {
    let (clause_il, is_else) = elab_clause(ctx, def)?;
    let def = def.node;
    if is_else {
        ctx.add_defined_func_else_clause(&def.id, clause_il)?;
    } else {
        ctx.add_defined_func_clause(&def.id, clause_il);
    }
    Ok(())
}

// == Specification

// - Definition population

/// Moves the collected rule groups of a relation into its IL definition.
fn populate_rel(ctx: &mut Context, rel_def_il: il::RelDef) -> il::RelDef {
    match rel_def_il {
        il::RelDef::Extern(_) => rel_def_il,
        il::RelDef::Defined(mut defined_rel_il) => {
            // elab_rel_def constructs an empty declaration
            assert!(defined_rel_il.rule_groups.is_empty() && defined_rel_il.else_group.is_none());

            // The collected rule groups replace the empty declaration
            let defined_rel_stored_il = ctx.take_defined_rel(&defined_rel_il.id);
            defined_rel_il.rule_groups = defined_rel_stored_il.rule_groups;
            defined_rel_il.else_group = defined_rel_stored_il.else_group;
            il::RelDef::Defined(defined_rel_il)
        }
    }
}

/// Moves the collected rows or clauses of a function into its IL definition.
fn populate_meta_func(ctx: &mut Context, meta_func_def_il: il::MetaFuncDef) -> il::MetaFuncDef {
    match meta_func_def_il {
        // Extern and builtin functions have no body to fill
        il::MetaFuncDef::Extern(_) => meta_func_def_il,
        il::MetaFuncDef::Builtin(_) => meta_func_def_il,
        il::MetaFuncDef::Table(mut table_func_il) => {
            // elab_table_dec_def constructs an empty declaration
            assert!(table_func_il.rows.is_empty());

            // The collected rows replace the empty declaration
            let table_func_stored_il = ctx.take_table_func(&table_func_il.id);
            table_func_il.rows = table_func_stored_il.rows;
            il::MetaFuncDef::Table(table_func_il)
        }
        il::MetaFuncDef::Defined(mut defined_func_il) => {
            // elab_func_dec_def constructs an empty declaration
            assert!(defined_func_il.clauses.is_empty() && defined_func_il.else_clause.is_none());

            // The collected clauses replace the empty declaration
            let defined_func_stored_il = ctx.take_defined_func(&defined_func_il.id);
            defined_func_il.clauses = defined_func_stored_il.clauses;
            defined_func_il.else_clause = defined_func_stored_il.else_clause;
            il::MetaFuncDef::Defined(defined_func_il)
        }
    }
}

/// Fills every declaration with the bodies collected in the context.
fn populate_defs(mut ctx: Context, defs_il: il::Spec) -> il::Spec {
    defs_il
        .into_iter()
        .map(|def_il| {
            // Only relations and functions collect bodies
            let def_kind_il = match def_il.node {
                il::DefKind::Rel(rel_def_il) => {
                    il::DefKind::Rel(populate_rel(&mut ctx, rel_def_il))
                }
                il::DefKind::MetaFunc(meta_func_def_il) => {
                    il::DefKind::MetaFunc(populate_meta_func(&mut ctx, meta_func_def_il))
                }
                def_kind_il => def_kind_il,
            };
            phrase!(node: def_kind_il, span: def_il.span)
        })
        .collect()
}

// - Missing definition warnings

/// Collects missing-body warnings for relations before functions.
fn warn_undef_defs(defs_il: &[il::Def], warnings: &mut Vec<Report>) {
    // Report missing relation bodies before missing function bodies
    for def_il in defs_il {
        if let il::DefKind::Rel(il::RelDef::Defined(rel_il)) = &def_il.node
            && rel_il.rule_groups.is_empty()
            && rel_il.else_group.is_none()
        {
            warnings.push(error::relation_rule_missing(&rel_il.id, &def_il.span));
        }
    }
    // Report missing table rows and function clauses in declaration order
    for def_il in defs_il {
        match &def_il.node {
            // Empty tables remain valid declarations
            il::DefKind::MetaFunc(il::MetaFuncDef::Table(func_il)) if func_il.rows.is_empty() => {
                warnings.push(error::table_row_missing(&func_il.id, &def_il.span));
            }
            // An otherwise clause is a body even without regular clauses
            il::DefKind::MetaFunc(il::MetaFuncDef::Defined(func_il))
                if func_il.clauses.is_empty() && func_il.else_clause.is_none() =>
            {
                warnings.push(error::function_clause_missing(&func_il.id, &def_il.span));
            }
            // Other definitions have no missing body warning
            _ => {}
        }
    }
}

// - Entry point

/// Elaborates a specification: definitions, population, dimension analysis.
pub(super) fn elab_spec(
    spec_el: el::Spec,
    warnings: &mut Vec<Report>,
) -> Result<il::Spec, ElabError> {
    let mut ctx = Context::new();
    let mut defs_il = Vec::new();
    // Declarations become IL definitions, bodies are collected in the context
    for def_el in spec_el {
        if let Some(def_il) = elab_def(&mut ctx, def_el)? {
            defs_il.push(def_il);
        }
    }
    // Attach the collected bodies to their declarations
    let mut defs_il = populate_defs(ctx, defs_il);
    // Retain missing-body warnings even if dimension analysis fails
    warn_undef_defs(&defs_il, warnings);
    // Annotate iterations with the variables they range over
    dimension::analyze_spec(&mut defs_il)?;
    Ok(defs_il)
}
