//! Prepared PL syntax without prose annotations
//!
//! `strip_exp` converts prepared PL expressions and assignment patterns
//! into the shared evaluator's AST, preserving slots, types, and spans.
//! A PL notation keeps its `Mixfix` form, so stripping interns it
//! into the run's `ShapeArena` and pairs the mixop with that shape,
//! with the argument expressions in notation order;
//! notation, path, and argument conversion leaves PL source intact.

use std::rc::Rc;

use crate::lang::data::{
    shape::{MixopShape, ShapeArena},
    value::ValueError,
};

use crate::runtime::envs::interp::pl::ast_prepared as ast;

use crate::interp::shared::prepare::ast as shared_ast;

// = Expressions

/// Removes prose annotations while retaining executable syntax and source data.
///
/// Fails only when the shape arena runs out of handles.
pub(super) fn strip_exp(
    shapes: &mut ShapeArena,
    exp: &ast::Exp,
) -> Result<shared_ast::Exp, ValueError> {
    stacker::maybe_grow(64 * 1024, 1024 * 1024, || {
        use ast::ExpKind as P;
        use shared_ast::ExpKind as E;
        let node = match &exp.node.node {
            P::Bool(value) => E::Bool(*value),
            P::Num(num) => E::Num(num.clone()),
            P::Text(text) => E::Text(text.clone()),
            P::Id(id) => E::Id(id.clone()),
            P::Un(op, typ, exp) => E::Un(*op, *typ, Box::new(strip_exp(shapes, exp)?)),
            P::Bin(op, typ, exp_l, exp_r) => E::Bin(
                *op,
                *typ,
                Box::new(strip_exp(shapes, exp_l)?),
                Box::new(strip_exp(shapes, exp_r)?),
            ),
            P::Cmp(op, typ, exp_l, exp_r) => E::Cmp(
                *op,
                *typ,
                Box::new(strip_exp(shapes, exp_l)?),
                Box::new(strip_exp(shapes, exp_r)?),
            ),
            P::UpCast(typ, exp) => {
                E::UpCast(Box::new(typ.clone()), Box::new(strip_exp(shapes, exp)?))
            }
            P::DownCast(typ, exp) => {
                E::DownCast(Box::new(typ.clone()), Box::new(strip_exp(shapes, exp)?))
            }
            P::Sub(exp, typ, check) => {
                E::Sub(Box::new(strip_exp(shapes, exp)?), Box::new(typ.clone()), check.clone())
            }
            P::Match(exp, pattern) => E::Match(Box::new(strip_exp(shapes, exp)?), pattern.clone()),
            P::Tuple(exps) => E::Tuple(
                exps.iter()
                    .map(|exp| strip_exp(shapes, exp))
                    .collect::<Result<_, _>>()?,
            ),
            P::Case(not_exp) => E::Case(Box::new(strip_not_exp(shapes, not_exp)?)),
            P::Str(fields) => E::Str(
                fields
                    .iter()
                    .map(|(atom, exp)| {
                        let exp = strip_exp(shapes, exp)?;
                        Ok(shared_ast::ExpField { atom: atom.clone(), exp })
                    })
                    .collect::<Result<_, ValueError>>()?,
            ),
            P::Opt(exp) => E::Opt(
                exp.as_ref()
                    .map(|exp| strip_exp(shapes, exp).map(Box::new))
                    .transpose()?,
            ),
            P::List(exps) => E::List(
                exps.iter()
                    .map(|exp| strip_exp(shapes, exp))
                    .collect::<Result<_, _>>()?,
            ),
            P::Cons(exp_head, exp_tail) => E::Cons(
                Box::new(strip_exp(shapes, exp_head)?),
                Box::new(strip_exp(shapes, exp_tail)?),
            ),
            P::Cat(exp_l, exp_r) => {
                E::Cat(Box::new(strip_exp(shapes, exp_l)?), Box::new(strip_exp(shapes, exp_r)?))
            }
            P::Mem(exp_elem, exp_list) => E::Mem(
                Box::new(strip_exp(shapes, exp_elem)?),
                Box::new(strip_exp(shapes, exp_list)?),
            ),
            P::Len(exp) => E::Len(Box::new(strip_exp(shapes, exp)?)),
            P::Dot(exp, atom) => E::Dot(Box::new(strip_exp(shapes, exp)?), atom.clone()),
            P::Idx(exp_base, exp_idx) => E::Idx(
                Box::new(strip_exp(shapes, exp_base)?),
                Box::new(strip_exp(shapes, exp_idx)?),
            ),
            P::Slice(exp_base, exp_idx, exp_len) => E::Slice(
                Box::new(strip_exp(shapes, exp_base)?),
                Box::new(strip_exp(shapes, exp_idx)?),
                Box::new(strip_exp(shapes, exp_len)?),
            ),
            P::Upd(exp_base, path, exp_new) => E::Upd(
                Box::new(strip_exp(shapes, exp_base)?),
                Box::new(strip_path(shapes, path)?),
                Box::new(strip_exp(shapes, exp_new)?),
            ),
            P::Call(id, targs, args) => E::Call(
                id.clone(),
                targs.clone(),
                args.iter()
                    .map(|arg| strip_arg(shapes, arg))
                    .collect::<Result<_, _>>()?,
            ),
            P::Iter(exp, iter) => E::Iter(Box::new(strip_exp(shapes, exp)?), iter.clone()),
        };
        Ok(crate::note_phrase! {
            node: node,
            note: exp.node.note.clone(),
            span: exp.node.span.clone(),
        })
    })
}

// = Notation

/// Interns the notation's shape and strips its arguments in notation order.
fn strip_not_exp(
    shapes: &mut ShapeArena,
    not_exp: &ast::NotExp,
) -> Result<shared_ast::NotExp, ValueError> {
    let (shape, _) = shapes.intern_notation(not_exp)?;
    let mixop = Rc::new(not_exp.to_mixop());
    let exps = not_exp
        .args()
        .into_iter()
        .map(|exp| strip_exp(shapes, exp))
        .collect::<Result<_, _>>()?;
    Ok(shared_ast::NotExp { notation: MixopShape { mixop, shape }, exps })
}

// = Paths

fn strip_path(shapes: &mut ShapeArena, path: &ast::Path) -> Result<shared_ast::Path, ValueError> {
    let node = match &path.node {
        ast::PathKind::Root => shared_ast::PathKind::Root,
        ast::PathKind::Idx(path, exp) => shared_ast::PathKind::Idx(
            Box::new(strip_path(shapes, path)?),
            Box::new(strip_exp(shapes, exp)?),
        ),
        ast::PathKind::Slice(path, exp_idx, exp_len) => shared_ast::PathKind::Slice(
            Box::new(strip_path(shapes, path)?),
            Box::new(strip_exp(shapes, exp_idx)?),
            Box::new(strip_exp(shapes, exp_len)?),
        ),
        ast::PathKind::Dot(path, atom) => {
            shared_ast::PathKind::Dot(Box::new(strip_path(shapes, path)?), atom.clone())
        }
    };
    Ok(crate::note_phrase!(node: node, note: path.note.clone(), span: path.span.clone()))
}

// = Arguments

fn strip_arg(shapes: &mut ShapeArena, arg: &ast::Arg) -> Result<shared_ast::Arg, ValueError> {
    let node = match &arg.node {
        ast::ArgKind::Exp(exp) => shared_ast::ArgKind::Exp(Box::new(strip_exp(shapes, exp)?)),
        ast::ArgKind::Def(id) => shared_ast::ArgKind::Def(id.clone()),
    };
    Ok(crate::phrase!(node: node, span: arg.span.clone()))
}
