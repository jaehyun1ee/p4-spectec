//! Direct SL-to-PL conversion with fused continuation linearization

use crate::lang::{
    al,
    common::{ds::set::IdSet, notation::mixfix::Mixfix, source::Span},
    hints::{alter, fields, input},
    il::ast as il,
    pl::{annot, ast as pl},
    sl::ast as sl,
};

use super::{Context, ProseError, ProseErrorKind};

// == Hint construction

fn build_func_hints(ctx: &Context, id_func: &sl::Id) -> annot::Hints {
    ctx.hints_func(id_func)
        .map(|hints_func| annot::Hints {
            prose_in: hints_func.prose_in.clone(),
            prose_true: hints_func.prose_true.clone(),
            prose_false: hints_func.prose_false.clone(),
            ..annot::Hints::default()
        })
        .unwrap_or_default()
}

fn build_rel_hints(
    ctx: &Context,
    id_rel: &sl::Id,
    rel_signature: &sl::RelSignature,
) -> Result<annot::Hints, ProseError> {
    let hints_default = annot::Hints::default();
    let hints_rel = ctx.hints_rel(id_rel).unwrap_or(&hints_default);
    let prose_out = hints_rel
        .prose_out
        .as_ref()
        .map(|hint| alter::realign(hint, &rel_signature.input_hint));
    let (prose_input_exps, prose_output_exps) = if hints_rel.prose_in.is_some() {
        let typs = rel_signature
            .not_typ
            .node
            .args()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let (typs_input, typs_output) = input::split(&rel_signature.input_hint, typs)
            .map_err(|error| ProseError::new(ProseErrorKind::Input(error), id_rel.span.clone()))?;
        let fresh_exps_from_typs = |typs: Vec<sl::Typ>| {
            let mut ids_used = IdSet::new();
            typs.into_iter()
                .map(|typ| {
                    let (ids_fresh, exp_sl) =
                        al::fresh::exp_from_typ(true, ctx.menv(), &ids_used, &typ);
                    ids_used = ids_fresh;
                    exp_sl
                })
                .collect::<Vec<_>>()
        };
        let exps_input_sl = Some(fresh_exps_from_typs(typs_input));
        let exps_output_sl = prose_out
            .is_some()
            .then(|| fresh_exps_from_typs(typs_output));
        (exps_input_sl, exps_output_sl)
    } else {
        (None, None)
    };
    Ok(annot::Hints {
        prose: hints_rel.prose.clone(),
        prose_in: hints_rel.prose_in.clone(),
        prose_out,
        prose_true: hints_rel.prose_true.clone(),
        prose_false: hints_rel.prose_false.clone(),
        prose_input_exps,
        prose_output_exps,
        prose_fields: None,
    })
}

// == Hint validation

fn validate_hint_alter(
    span: &Span,
    hints: &annot::Hints,
    num_items: usize,
) -> Result<(), ProseError> {
    for hint in
        [&hints.prose, &hints.prose_in, &hints.prose_out, &hints.prose_true, &hints.prose_false]
            .into_iter()
            .flatten()
    {
        alter::validate_count(hint, num_items)
            .map_err(|error| ProseError::new(ProseErrorKind::Alteration(error), span.clone()))?;
    }
    Ok(())
}

fn validate_hint_fields(
    span: &Span,
    hints: &annot::Hints,
    num_fields: usize,
) -> Result<(), ProseError> {
    if let Some(hint) = &hints.prose_fields {
        fields::validate(hint, num_fields)
            .map_err(|error| ProseError::new(ProseErrorKind::Field(error), span.clone()))?;
    }
    Ok(())
}

fn validate_hint_split(
    span: &Span,
    hints: &annot::Hints,
    num_inputs: usize,
    num_outputs: usize,
) -> Result<(), ProseError> {
    if let Some(hint) = &hints.prose_in {
        alter::validate_count(hint, num_inputs)
            .map_err(|error| ProseError::new(ProseErrorKind::Alteration(error), span.clone()))?;
    }
    if let Some(hint) = &hints.prose_out {
        alter::validate_count(hint, num_outputs)
            .map_err(|error| ProseError::new(ProseErrorKind::Alteration(error), span.clone()))?;
    }
    Ok(())
}

// == Expressions

// - Expression

fn prosify_exp(ctx: &Context, exp_sl: &sl::Exp) -> Result<pl::Exp, ProseError> {
    let (exp_kind_pl, hints) = prosify_exp_kind(ctx, exp_sl)?;
    let exp_node_pl = crate::note_phrase! {
        node: exp_kind_pl,
        note: exp_sl.note.as_ref().clone(),
        span: exp_sl.span.clone(),
    };
    let exp_pl = crate::annotated! { node: exp_node_pl, hints: hints };
    Ok(exp_pl)
}

fn prosify_exp_kind(
    ctx: &Context,
    exp_sl: &sl::Exp,
) -> Result<(pl::ExpKind, annot::Hints), ProseError> {
    let exp_kind_pl = match &exp_sl.node {
        il::ExpKind::Bool(value) => pl::ExpKind::Bool(*value),
        il::ExpKind::Num(num) => pl::ExpKind::Num(num.clone()),
        il::ExpKind::Text(text) => pl::ExpKind::Text(text.clone()),
        il::ExpKind::Var(id) => pl::ExpKind::Var(id.clone()),
        il::ExpKind::Un(op, op_typ, exp_inner_sl) => {
            prosify_un_exp(ctx, *op, *op_typ, exp_inner_sl)?
        }
        il::ExpKind::Bin(op, op_typ, exp_l_sl, exp_r_sl) => {
            prosify_bin_exp(ctx, *op, *op_typ, exp_l_sl, exp_r_sl)?
        }
        il::ExpKind::Cmp(op, op_typ, exp_l_sl, exp_r_sl) => {
            prosify_cmp_exp(ctx, *op, *op_typ, exp_l_sl, exp_r_sl)?
        }
        il::ExpKind::UpCast(typ, exp_inner_sl) => prosify_upcast_exp(ctx, typ, exp_inner_sl)?,
        il::ExpKind::DownCast(typ, exp_inner_sl) => prosify_downcast_exp(ctx, typ, exp_inner_sl)?,
        il::ExpKind::Sub(exp_inner_sl, typ, subcheck) => {
            prosify_sub_exp(ctx, exp_inner_sl, typ, subcheck)?
        }
        il::ExpKind::Match(exp_inner_sl, pattern) => prosify_match_exp(ctx, exp_inner_sl, pattern)?,
        il::ExpKind::Tuple(exps_sl) => prosify_tuple_exp(ctx, exps_sl)?,
        il::ExpKind::Case(not_exp_sl) => return prosify_case_exp(ctx, exp_sl, not_exp_sl),
        il::ExpKind::Str(fields_sl) => prosify_struct_exp(ctx, fields_sl)?,
        il::ExpKind::Opt(exp_opt_sl) => prosify_option_exp(ctx, exp_opt_sl)?,
        il::ExpKind::List(exps_sl) => prosify_list_exp(ctx, exps_sl)?,
        il::ExpKind::Cons(exp_l_sl, exp_r_sl) => prosify_cons_exp(ctx, exp_l_sl, exp_r_sl)?,
        il::ExpKind::Cat(exp_l_sl, exp_r_sl) => prosify_cat_exp(ctx, exp_l_sl, exp_r_sl)?,
        il::ExpKind::Mem(exp_l_sl, exp_r_sl) => prosify_mem_exp(ctx, exp_l_sl, exp_r_sl)?,
        il::ExpKind::Len(exp_inner_sl) => prosify_len_exp(ctx, exp_inner_sl)?,
        il::ExpKind::Dot(exp_inner_sl, atom) => prosify_dot_exp(ctx, exp_inner_sl, atom)?,
        il::ExpKind::Idx(exp_l_sl, exp_r_sl) => prosify_idx_exp(ctx, exp_l_sl, exp_r_sl)?,
        il::ExpKind::Slice(exp_base_sl, exp_idx_sl, exp_len_sl) => {
            prosify_slice_exp(ctx, exp_base_sl, exp_idx_sl, exp_len_sl)?
        }
        il::ExpKind::Upd(exp_base_sl, path_sl, exp_field_sl) => {
            prosify_update_exp(ctx, exp_base_sl, path_sl, exp_field_sl)?
        }
        il::ExpKind::Call(id, targs, args_sl) => {
            return prosify_call_exp(ctx, exp_sl, id, targs, args_sl);
        }
        il::ExpKind::Iter(exp_inner_sl, iter_exp) => prosify_iter_exp(ctx, exp_inner_sl, iter_exp)?,
    };
    Ok((exp_kind_pl, annot::Hints::default()))
}

// - Unary expression

fn prosify_un_exp(
    ctx: &Context,
    op: il::UnOp,
    op_typ: il::OpTyp,
    exp_inner_sl: &sl::Exp,
) -> Result<pl::ExpKind, ProseError> {
    let exp_inner_pl = prosify_exp(ctx, exp_inner_sl)?;
    Ok(pl::ExpKind::Un(op, op_typ, Box::new(exp_inner_pl)))
}

// - Binary expression

fn prosify_bin_exp(
    ctx: &Context,
    op: il::BinOp,
    op_typ: il::OpTyp,
    exp_l_sl: &sl::Exp,
    exp_r_sl: &sl::Exp,
) -> Result<pl::ExpKind, ProseError> {
    let exp_l_pl = prosify_exp(ctx, exp_l_sl)?;
    let exp_r_pl = prosify_exp(ctx, exp_r_sl)?;
    Ok(pl::ExpKind::Bin(op, op_typ, Box::new(exp_l_pl), Box::new(exp_r_pl)))
}

// - Comparison expression

fn prosify_cmp_exp(
    ctx: &Context,
    op: il::CmpOp,
    op_typ: il::OpTyp,
    exp_l_sl: &sl::Exp,
    exp_r_sl: &sl::Exp,
) -> Result<pl::ExpKind, ProseError> {
    let exp_l_pl = prosify_exp(ctx, exp_l_sl)?;
    let exp_r_pl = prosify_exp(ctx, exp_r_sl)?;
    Ok(pl::ExpKind::Cmp(op, op_typ, Box::new(exp_l_pl), Box::new(exp_r_pl)))
}

// - Upcast expression

fn prosify_upcast_exp(
    ctx: &Context,
    typ: &sl::Typ,
    exp_inner_sl: &sl::Exp,
) -> Result<pl::ExpKind, ProseError> {
    let exp_inner_pl = prosify_exp(ctx, exp_inner_sl)?;
    Ok(pl::ExpKind::UpCast(typ.clone(), Box::new(exp_inner_pl)))
}

// - Downcast expression

fn prosify_downcast_exp(
    ctx: &Context,
    typ: &sl::Typ,
    exp_inner_sl: &sl::Exp,
) -> Result<pl::ExpKind, ProseError> {
    let exp_inner_pl = prosify_exp(ctx, exp_inner_sl)?;
    Ok(pl::ExpKind::DownCast(typ.clone(), Box::new(exp_inner_pl)))
}

// - Subtype expression

fn prosify_sub_exp(
    ctx: &Context,
    exp_inner_sl: &sl::Exp,
    typ: &sl::Typ,
    subcheck: &sl::Subcheck,
) -> Result<pl::ExpKind, ProseError> {
    let exp_inner_pl = prosify_exp(ctx, exp_inner_sl)?;
    Ok(pl::ExpKind::Sub(Box::new(exp_inner_pl), typ.clone(), Box::new(subcheck.clone())))
}

// - Match expression

fn prosify_match_exp(
    ctx: &Context,
    exp_inner_sl: &sl::Exp,
    pattern: &sl::Pattern,
) -> Result<pl::ExpKind, ProseError> {
    let exp_inner_pl = prosify_exp(ctx, exp_inner_sl)?;
    Ok(pl::ExpKind::Match(Box::new(exp_inner_pl), pattern.clone()))
}

// - Tuple expression

fn prosify_tuple_exp(ctx: &Context, exps_sl: &[sl::Exp]) -> Result<pl::ExpKind, ProseError> {
    let exps_pl = prosify_exps(ctx, exps_sl)?;
    Ok(pl::ExpKind::Tuple(exps_pl))
}

// - Case expression

fn prosify_case_exp(
    ctx: &Context,
    exp_sl: &sl::Exp,
    not_exp_sl: &sl::NotExp,
) -> Result<(pl::ExpKind, annot::Hints), ProseError> {
    let not_exp_pl = prosify_not_exp(ctx, not_exp_sl)?;
    let hints = match exp_sl.note.as_ref() {
        il::TypKind::Var(id_typ, _) => ctx
            .hints_case(id_typ, &not_exp_sl.to_mixop())
            .map(|hints_case| annot::Hints {
                prose: hints_case.prose.clone(),
                prose_fields: hints_case.prose_fields.clone(),
                ..annot::Hints::default()
            })
            .unwrap_or_default(),
        _ => annot::Hints::default(),
    };
    let num_args = not_exp_sl.args().len();
    validate_hint_alter(&exp_sl.span, &hints, num_args)?;
    validate_hint_fields(&exp_sl.span, &hints, num_args)?;
    Ok((pl::ExpKind::Case(Box::new(not_exp_pl)), hints))
}

// - Struct expression

fn prosify_struct_exp(
    ctx: &Context,
    fields_sl: &[il::ExpField],
) -> Result<pl::ExpKind, ProseError> {
    let mut fields_pl = Vec::with_capacity(fields_sl.len());
    for (atom, exp_sl) in fields_sl {
        let exp_pl = prosify_exp(ctx, exp_sl)?;
        fields_pl.push((atom.clone(), exp_pl));
    }
    Ok(pl::ExpKind::Str(fields_pl))
}

// - Optional expression

fn prosify_option_exp(
    ctx: &Context,
    exp_opt_sl: &Option<Box<sl::Exp>>,
) -> Result<pl::ExpKind, ProseError> {
    let exp_opt_pl = match exp_opt_sl.as_deref() {
        Some(exp_sl) => {
            let exp_pl = prosify_exp(ctx, exp_sl)?;
            Some(Box::new(exp_pl))
        }
        None => None,
    };
    Ok(pl::ExpKind::Opt(exp_opt_pl))
}

// - List expression

fn prosify_list_exp(ctx: &Context, exps_sl: &[sl::Exp]) -> Result<pl::ExpKind, ProseError> {
    let exps_pl = prosify_exps(ctx, exps_sl)?;
    Ok(pl::ExpKind::List(exps_pl))
}

// - Cons expression

fn prosify_cons_exp(
    ctx: &Context,
    exp_l_sl: &sl::Exp,
    exp_r_sl: &sl::Exp,
) -> Result<pl::ExpKind, ProseError> {
    let exp_l_pl = prosify_exp(ctx, exp_l_sl)?;
    let exp_r_pl = prosify_exp(ctx, exp_r_sl)?;
    Ok(pl::ExpKind::Cons(Box::new(exp_l_pl), Box::new(exp_r_pl)))
}

// - Concatenation expression

fn prosify_cat_exp(
    ctx: &Context,
    exp_l_sl: &sl::Exp,
    exp_r_sl: &sl::Exp,
) -> Result<pl::ExpKind, ProseError> {
    let exp_l_pl = prosify_exp(ctx, exp_l_sl)?;
    let exp_r_pl = prosify_exp(ctx, exp_r_sl)?;
    Ok(pl::ExpKind::Cat(Box::new(exp_l_pl), Box::new(exp_r_pl)))
}

// - Membership expression

fn prosify_mem_exp(
    ctx: &Context,
    exp_l_sl: &sl::Exp,
    exp_r_sl: &sl::Exp,
) -> Result<pl::ExpKind, ProseError> {
    let exp_l_pl = prosify_exp(ctx, exp_l_sl)?;
    let exp_r_pl = prosify_exp(ctx, exp_r_sl)?;
    Ok(pl::ExpKind::Mem(Box::new(exp_l_pl), Box::new(exp_r_pl)))
}

// - Length expression

fn prosify_len_exp(ctx: &Context, exp_inner_sl: &sl::Exp) -> Result<pl::ExpKind, ProseError> {
    let exp_inner_pl = prosify_exp(ctx, exp_inner_sl)?;
    Ok(pl::ExpKind::Len(Box::new(exp_inner_pl)))
}

// - Dot expression

fn prosify_dot_exp(
    ctx: &Context,
    exp_inner_sl: &sl::Exp,
    atom: &il::Atom,
) -> Result<pl::ExpKind, ProseError> {
    let exp_inner_pl = prosify_exp(ctx, exp_inner_sl)?;
    Ok(pl::ExpKind::Dot(Box::new(exp_inner_pl), atom.clone()))
}

// - Index expression

fn prosify_idx_exp(
    ctx: &Context,
    exp_l_sl: &sl::Exp,
    exp_r_sl: &sl::Exp,
) -> Result<pl::ExpKind, ProseError> {
    let exp_l_pl = prosify_exp(ctx, exp_l_sl)?;
    let exp_r_pl = prosify_exp(ctx, exp_r_sl)?;
    Ok(pl::ExpKind::Idx(Box::new(exp_l_pl), Box::new(exp_r_pl)))
}

// - Slice expression

fn prosify_slice_exp(
    ctx: &Context,
    exp_base_sl: &sl::Exp,
    exp_idx_sl: &sl::Exp,
    exp_len_sl: &sl::Exp,
) -> Result<pl::ExpKind, ProseError> {
    let exp_base_pl = prosify_exp(ctx, exp_base_sl)?;
    let exp_idx_pl = prosify_exp(ctx, exp_idx_sl)?;
    let exp_len_pl = prosify_exp(ctx, exp_len_sl)?;
    Ok(pl::ExpKind::Slice(Box::new(exp_base_pl), Box::new(exp_idx_pl), Box::new(exp_len_pl)))
}

// - Update expression

fn prosify_update_exp(
    ctx: &Context,
    exp_base_sl: &sl::Exp,
    path_sl: &sl::Path,
    exp_field_sl: &sl::Exp,
) -> Result<pl::ExpKind, ProseError> {
    let exp_base_pl = prosify_exp(ctx, exp_base_sl)?;
    let path_pl = prosify_path(ctx, path_sl)?;
    let exp_field_pl = prosify_exp(ctx, exp_field_sl)?;
    Ok(pl::ExpKind::Upd(Box::new(exp_base_pl), Box::new(path_pl), Box::new(exp_field_pl)))
}

// - Call expression

fn prosify_call_exp(
    ctx: &Context,
    exp_sl: &sl::Exp,
    id_func: &sl::Id,
    targs: &[sl::Targ],
    args_sl: &[sl::Arg],
) -> Result<(pl::ExpKind, annot::Hints), ProseError> {
    let args_pl = prosify_args(ctx, args_sl)?;
    let hints = build_func_hints(ctx, id_func);
    validate_hint_alter(&exp_sl.span, &hints, args_sl.len())?;
    Ok((pl::ExpKind::Call(id_func.clone(), targs.to_vec(), args_pl), hints))
}

// - Iterated expression

fn prosify_iter_exp(
    ctx: &Context,
    exp_inner_sl: &sl::Exp,
    iter_exp: &sl::ExpIter,
) -> Result<pl::ExpKind, ProseError> {
    let exp_inner_pl = prosify_exp(ctx, exp_inner_sl)?;
    Ok(pl::ExpKind::Iter(Box::new(exp_inner_pl), iter_exp.clone()))
}

// - Expression list

fn prosify_exps(ctx: &Context, exps_sl: &[sl::Exp]) -> Result<Vec<pl::Exp>, ProseError> {
    let mut exps_pl = Vec::with_capacity(exps_sl.len());
    for exp_sl in exps_sl {
        let exp_pl = prosify_exp(ctx, exp_sl)?;
        exps_pl.push(exp_pl);
    }
    Ok(exps_pl)
}

// - Notation expression

fn prosify_not_exp(ctx: &Context, not_exp_sl: &sl::NotExp) -> Result<pl::NotExp, ProseError> {
    let not_exp_pl = match not_exp_sl {
        Mixfix::Arg(exp_sl) => {
            let exp_pl = prosify_exp(ctx, exp_sl)?;
            Mixfix::Arg(exp_pl)
        }
        Mixfix::Atom(atom) => Mixfix::Atom(atom.clone()),
        Mixfix::Brack(atom_l, not_exp_inner_sl, atom_r) => {
            let not_exp_inner_pl = prosify_not_exp(ctx, not_exp_inner_sl)?;
            Mixfix::Brack(atom_l.clone(), Box::new(not_exp_inner_pl), atom_r.clone())
        }
        Mixfix::Infix(not_exp_l_sl, atom, not_exp_r_sl) => {
            let not_exp_l_pl = prosify_not_exp(ctx, not_exp_l_sl)?;
            let not_exp_r_pl = prosify_not_exp(ctx, not_exp_r_sl)?;
            Mixfix::Infix(Box::new(not_exp_l_pl), atom.clone(), Box::new(not_exp_r_pl))
        }
        Mixfix::Seq(not_exps_sl) => {
            let mut not_exps_pl = Vec::with_capacity(not_exps_sl.len());
            for not_exp_sl in not_exps_sl {
                let not_exp_pl = prosify_not_exp(ctx, not_exp_sl)?;
                not_exps_pl.push(not_exp_pl);
            }
            Mixfix::Seq(not_exps_pl)
        }
    };
    Ok(not_exp_pl)
}

// == Paths

fn prosify_path(ctx: &Context, path_sl: &sl::Path) -> Result<pl::Path, ProseError> {
    let path_kind_pl = prosify_path_kind(ctx, &path_sl.node)?;
    let path_pl = crate::note_phrase! {
        node: path_kind_pl,
        note: path_sl.note.as_ref().clone(),
        span: path_sl.span.clone(),
    };
    Ok(path_pl)
}

fn prosify_path_kind(
    ctx: &Context,
    path_kind_sl: &sl::PathKind,
) -> Result<pl::PathKind, ProseError> {
    let path_kind_pl = match path_kind_sl {
        il::PathKind::Root => pl::PathKind::Root,
        il::PathKind::Idx(path_inner_sl, exp_idx_sl) => {
            let path_inner_pl = prosify_path(ctx, path_inner_sl)?;
            let exp_idx_pl = prosify_exp(ctx, exp_idx_sl)?;
            pl::PathKind::Idx(Box::new(path_inner_pl), Box::new(exp_idx_pl))
        }
        il::PathKind::Slice(path_inner_sl, exp_idx_sl, exp_len_sl) => {
            let path_inner_pl = prosify_path(ctx, path_inner_sl)?;
            let exp_idx_pl = prosify_exp(ctx, exp_idx_sl)?;
            let exp_len_pl = prosify_exp(ctx, exp_len_sl)?;
            pl::PathKind::Slice(Box::new(path_inner_pl), Box::new(exp_idx_pl), Box::new(exp_len_pl))
        }
        il::PathKind::Dot(path_inner_sl, atom) => {
            let path_inner_pl = prosify_path(ctx, path_inner_sl)?;
            pl::PathKind::Dot(Box::new(path_inner_pl), atom.clone())
        }
    };
    Ok(path_kind_pl)
}

// == Arguments

fn prosify_arg(ctx: &Context, arg_sl: &sl::Arg) -> Result<pl::Arg, ProseError> {
    let arg_kind_pl = prosify_arg_kind(ctx, &arg_sl.node)?;
    let arg_pl = crate::phrase! { node: arg_kind_pl, span: arg_sl.span.clone() };
    Ok(arg_pl)
}

fn prosify_arg_kind(ctx: &Context, arg_kind_sl: &sl::ArgKind) -> Result<pl::ArgKind, ProseError> {
    let arg_kind_pl = match arg_kind_sl {
        il::ArgKind::Exp(exp_sl) => {
            let exp_pl = prosify_exp(ctx, exp_sl)?;
            pl::ArgKind::Exp(Box::new(exp_pl))
        }
        il::ArgKind::Def(id) => pl::ArgKind::Def(id.clone()),
    };
    Ok(arg_kind_pl)
}

fn prosify_args(ctx: &Context, args_sl: &[sl::Arg]) -> Result<Vec<pl::Arg>, ProseError> {
    let mut args_pl = Vec::with_capacity(args_sl.len());
    for arg_sl in args_sl {
        let arg_pl = prosify_arg(ctx, arg_sl)?;
        args_pl.push(arg_pl);
    }
    Ok(args_pl)
}

// == Parameters

fn prosify_param(ctx: &Context, param_sl: &sl::Param) -> Result<pl::Param, ProseError> {
    let param_kind_pl = prosify_param_kind(ctx, &param_sl.node)?;
    let param_pl = crate::phrase! { node: param_kind_pl, span: param_sl.span.clone() };
    Ok(param_pl)
}

fn prosify_param_kind(
    ctx: &Context,
    param_kind_sl: &sl::ParamKind,
) -> Result<pl::ParamKind, ProseError> {
    let param_kind_pl = match param_kind_sl {
        sl::ParamKind::Exp(typ, exp_sl) => {
            let exp_pl = prosify_exp(ctx, exp_sl)?;
            pl::ParamKind::Exp(typ.clone(), Box::new(exp_pl))
        }
        sl::ParamKind::Def(id, tparams, params_sl, typ) => {
            let params_pl = prosify_params(ctx, params_sl)?;
            pl::ParamKind::Def(id.clone(), tparams.clone(), params_pl, typ.clone())
        }
    };
    Ok(param_kind_pl)
}

fn prosify_params(ctx: &Context, params_sl: &[sl::Param]) -> Result<Vec<pl::Param>, ProseError> {
    let mut params_pl = Vec::with_capacity(params_sl.len());
    for param_sl in params_sl {
        let param_pl = prosify_param(ctx, param_sl)?;
        params_pl.push(param_pl);
    }
    Ok(params_pl)
}

// == Guards

fn prosify_guard(ctx: &Context, guard_sl: &sl::Guard) -> Result<pl::Guard, ProseError> {
    let guard_pl = match guard_sl {
        sl::Guard::Bool(value) => pl::Guard::Bool(*value),
        sl::Guard::Cmp(op, op_typ, exp_sl) => {
            let exp_pl = prosify_exp(ctx, exp_sl)?;
            pl::Guard::Cmp(*op, *op_typ, exp_pl)
        }
        sl::Guard::Sub(typ, subcheck) => pl::Guard::Sub(typ.clone(), subcheck.clone()),
        sl::Guard::Match(pattern) => pl::Guard::Match(pattern.clone()),
        sl::Guard::Mem(exp_sl) => {
            let exp_pl = prosify_exp(ctx, exp_sl)?;
            pl::Guard::Mem(exp_pl)
        }
    };
    Ok(guard_pl)
}

// == Instruction construction

fn make_instr<Tier>(instr_kind_pl: pl::InstrKind<Tier>, span: Span) -> pl::Instr<Tier> {
    make_instr_with_hints(instr_kind_pl, span, annot::Hints::default())
}

fn make_instr_with_hints<Tier>(
    instr_kind_pl: pl::InstrKind<Tier>,
    span: Span,
    hints: annot::Hints,
) -> pl::Instr<Tier> {
    let instr_node_pl = crate::note_phrase! {
        node: instr_kind_pl,
        note: None,
        span: span,
    };
    crate::annotated! { node: instr_node_pl, hints: hints }
}

// == Dispatch instructions

// - Instruction

fn prosify_dispatch_instr(
    ctx: &Context,
    instr_sl: sl::Instr,
) -> Result<pl::DispatchBlock, ProseError> {
    prosify_dispatch_instr_kind(ctx, instr_sl.node, instr_sl.span)
}

fn prosify_dispatch_instr_kind(
    ctx: &Context,
    instr_kind_sl: sl::InstrKind,
    span: Span,
) -> Result<pl::DispatchBlock, ProseError> {
    match instr_kind_sl {
        sl::InstrKind::If(instr_sl) => prosify_dispatch_if_instr(ctx, instr_sl, span),
        sl::InstrKind::Hold(instr_sl) => prosify_dispatch_hold_instr(ctx, instr_sl, span),
        sl::InstrKind::Case(instr_sl) => prosify_dispatch_case_instr(ctx, instr_sl, span),
        sl::InstrKind::Let(instr_sl) => prosify_dispatch_let_instr(ctx, instr_sl, span),
        sl::InstrKind::Debug(instr_sl) => prosify_dispatch_debug_instr(ctx, instr_sl, span),
        sl::InstrKind::Group(instr_sl) => prosify_dispatch_rulegroup_instr(ctx, instr_sl, span),
        sl::InstrKind::Rule(_) | sl::InstrKind::Result(_) | sl::InstrKind::Return(_) => {
            Err(ProseError::new(ProseErrorKind::InvalidDispatchTier, span))
        }
    }
}

// - If instruction

fn prosify_dispatch_if_instr(
    ctx: &Context,
    instr_sl: sl::IfInstr,
    span: Span,
) -> Result<pl::DispatchBlock, ProseError> {
    let exp_pl = prosify_exp(ctx, &instr_sl.exp)?;
    let block_pl = prosify_dispatch_block(ctx, instr_sl.block)?;
    let instr_pl = pl::IfInstr {
        exp: exp_pl,
        iter_exps: instr_sl.iter_exps,
        block: block_pl,
        dangle: instr_sl.dangle,
    };
    let instr_kind_pl = pl::InstrKind::If(instr_pl);
    let instr_pl = make_instr(instr_kind_pl, span);
    Ok(vec![instr_pl])
}

// - Hold instruction

fn prosify_dispatch_hold_instr(
    ctx: &Context,
    instr_sl: sl::HoldInstr,
    span: Span,
) -> Result<pl::DispatchBlock, ProseError> {
    let hints = ctx
        .hints_rel(&instr_sl.id)
        .map(|hints_rel| annot::Hints {
            prose_true: hints_rel.prose_true.clone(),
            prose_false: hints_rel.prose_false.clone(),
            ..annot::Hints::default()
        })
        .unwrap_or_default();
    validate_hint_alter(&span, &hints, instr_sl.not_exp.args().len())?;
    let not_exp_pl = prosify_not_exp(ctx, &instr_sl.not_exp)?;
    let hold_case_pl = prosify_dispatch_hold_case(ctx, instr_sl.hold_case)?;
    let instr_pl = pl::HoldInstr {
        id: instr_sl.id,
        not_exp: not_exp_pl,
        iter_exps: instr_sl.iter_exps,
        hold_case: hold_case_pl,
    };
    let instr_kind_pl = pl::InstrKind::Hold(instr_pl);
    let instr_pl = make_instr_with_hints(instr_kind_pl, span, hints);
    Ok(vec![instr_pl])
}

fn prosify_dispatch_hold_case(
    ctx: &Context,
    hold_case_sl: sl::HoldCase,
) -> Result<pl::HoldCase<pl::DispatchInstr>, ProseError> {
    let hold_case_pl = match hold_case_sl {
        sl::HoldCase::Both(block_hold_sl, block_not_hold_sl) => {
            let block_hold_pl = prosify_dispatch_block(ctx, block_hold_sl)?;
            let block_not_hold_pl = prosify_dispatch_block(ctx, block_not_hold_sl)?;
            pl::HoldCase::Both(block_hold_pl, block_not_hold_pl)
        }
        sl::HoldCase::Hold(block_sl, dangle) => {
            let block_pl = prosify_dispatch_block(ctx, block_sl)?;
            pl::HoldCase::Hold(block_pl, dangle)
        }
        sl::HoldCase::NotHold(block_sl, dangle) => {
            let block_pl = prosify_dispatch_block(ctx, block_sl)?;
            pl::HoldCase::NotHold(block_pl, dangle)
        }
    };
    Ok(hold_case_pl)
}

// - Case instruction

fn prosify_dispatch_case(
    ctx: &Context,
    case_sl: sl::Case,
) -> Result<pl::Case<pl::DispatchInstr>, ProseError> {
    let guard_pl = prosify_guard(ctx, &case_sl.guard)?;
    let block_pl = prosify_dispatch_block(ctx, case_sl.block)?;
    Ok(pl::Case { guard: guard_pl, block: block_pl })
}

fn prosify_dispatch_case_instr(
    ctx: &Context,
    instr_sl: sl::CaseInstr,
    span: Span,
) -> Result<pl::DispatchBlock, ProseError> {
    let exp_pl = prosify_exp(ctx, &instr_sl.exp)?;
    let mut cases_pl = Vec::with_capacity(instr_sl.cases.len());
    for case_sl in instr_sl.cases {
        let case_pl = prosify_dispatch_case(ctx, case_sl)?;
        cases_pl.push(case_pl);
    }
    let instr_pl = pl::CaseInstr { exp: exp_pl, cases: cases_pl, dangle: instr_sl.dangle };
    let instr_kind_pl = pl::InstrKind::Case(instr_pl);
    let instr_pl = make_instr(instr_kind_pl, span);
    Ok(vec![instr_pl])
}

// - Let instruction

fn prosify_dispatch_let_instr(
    ctx: &Context,
    instr_sl: sl::LetInstr,
    span: Span,
) -> Result<pl::DispatchBlock, ProseError> {
    let exp_l_pl = prosify_exp(ctx, &instr_sl.exp_l)?;
    let hints = if matches!(exp_l_pl.node.node, pl::ExpKind::Case(_)) {
        annot::Hints {
            prose_fields: exp_l_pl.hints.prose_fields.clone(),
            ..annot::Hints::default()
        }
    } else {
        annot::Hints::default()
    };
    let exp_r_pl = prosify_exp(ctx, &instr_sl.exp_r)?;
    let instr_pl =
        pl::LetInstr { exp_l: exp_l_pl, exp_r: exp_r_pl, iter_instrs: instr_sl.iter_instrs };
    let instr_kind_pl = pl::InstrKind::Let(instr_pl);
    let instr_pl = make_instr_with_hints(instr_kind_pl, span, hints);
    let block_pl = prosify_dispatch_block(ctx, instr_sl.block)?;
    let mut instrs_pl = vec![instr_pl];
    instrs_pl.extend(block_pl);
    Ok(instrs_pl)
}

// - Debug instruction

fn prosify_dispatch_debug_instr(
    ctx: &Context,
    instr_sl: sl::DebugInstr,
    span: Span,
) -> Result<pl::DispatchBlock, ProseError> {
    let exp_pl = prosify_exp(ctx, &instr_sl.exp)?;
    let instr_pl = pl::DebugInstr { exp: exp_pl };
    let instr_kind_pl = pl::InstrKind::Debug(instr_pl);
    let instr_pl = make_instr(instr_kind_pl, span);
    let instrs_follow_pl = prosify_dispatch_instr(ctx, *instr_sl.instr)?;
    let mut instrs_pl = vec![instr_pl];
    instrs_pl.extend(instrs_follow_pl);
    Ok(instrs_pl)
}

// - Group instruction

fn prosify_dispatch_rulegroup_instr(
    ctx: &Context,
    instr_sl: sl::GroupInstr,
    span: Span,
) -> Result<pl::DispatchBlock, ProseError> {
    let hints = ctx
        .hints_rel(ctx.namespace())
        .map(|hints_rel| annot::Hints {
            prose_in: hints_rel.prose_in.clone(),
            prose_true: hints_rel.prose_true.clone(),
            ..annot::Hints::default()
        })
        .unwrap_or_default();
    input::validate(&instr_sl.rel_signature.input_hint, instr_sl.exps.len())
        .map_err(|error| ProseError::new(ProseErrorKind::Input(error), span.clone()))?;
    validate_hint_alter(&span, &hints, instr_sl.rel_signature.input_hint.indices().len())?;
    let exps_input_pl = prosify_exps(ctx, &instr_sl.exps)?;
    let block_pl = prosify_group_block(ctx, instr_sl.block)?;
    let instr_pl = pl::RuleGroupInstr {
        id_rel: ctx.namespace().clone(),
        id_group: instr_sl.id,
        rel_signature: instr_sl.rel_signature,
        exps_input: exps_input_pl,
        block: block_pl,
    };
    let tier_pl = pl::DispatchInstr::Group(instr_pl);
    let instr_pl = pl::TierInstr { tier: tier_pl };
    let instr_kind_pl = pl::InstrKind::Tier(instr_pl);
    let instr_pl = make_instr_with_hints(instr_kind_pl, span, hints);
    Ok(vec![instr_pl])
}

// - Block

fn prosify_dispatch_block(
    ctx: &Context,
    block_sl: sl::Block,
) -> Result<pl::DispatchBlock, ProseError> {
    match block_sl.len() {
        0 => Ok(Vec::new()),
        1 => {
            let instr_sl = block_sl.into_iter().next().unwrap();
            prosify_dispatch_instr(ctx, instr_sl)
        }
        _ => {
            let spans = block_sl
                .iter()
                .map(|instr_sl| instr_sl.span.clone())
                .collect::<Vec<_>>();
            let span = Span::over(&spans);
            let mut blocks_pl = Vec::with_capacity(block_sl.len());
            for instr_sl in block_sl {
                let block_pl = prosify_dispatch_instr(ctx, instr_sl)?;
                blocks_pl.push(block_pl);
            }
            let instr_pl = pl::RouteInstr { blocks: blocks_pl };
            let tier_pl = pl::DispatchInstr::Route(instr_pl);
            let instr_pl = pl::TierInstr { tier: tier_pl };
            let instr_kind_pl = pl::InstrKind::Tier(instr_pl);
            let instr_pl = make_instr(instr_kind_pl, span);
            Ok(vec![instr_pl])
        }
    }
}

// == Group instructions

// - Instruction

fn prosify_group_instr(ctx: &Context, instr_sl: sl::Instr) -> Result<pl::GroupBlock, ProseError> {
    prosify_group_instr_kind(ctx, instr_sl.node, instr_sl.span)
}

fn prosify_group_instr_kind(
    ctx: &Context,
    instr_kind_sl: sl::InstrKind,
    span: Span,
) -> Result<pl::GroupBlock, ProseError> {
    match instr_kind_sl {
        sl::InstrKind::If(instr_sl) => prosify_group_if_instr(ctx, instr_sl, span),
        sl::InstrKind::Hold(instr_sl) => prosify_group_hold_instr(ctx, instr_sl, span),
        sl::InstrKind::Case(instr_sl) => prosify_group_case_instr(ctx, instr_sl, span),
        sl::InstrKind::Let(instr_sl) => prosify_group_let_instr(ctx, instr_sl, span),
        sl::InstrKind::Debug(instr_sl) => prosify_group_debug_instr(ctx, instr_sl, span),
        sl::InstrKind::Rule(instr_sl) => prosify_group_rule_instr(ctx, instr_sl, span),
        sl::InstrKind::Result(instr_sl) => prosify_group_result_instr(ctx, instr_sl, span),
        sl::InstrKind::Return(instr_sl) => prosify_group_return_instr(ctx, instr_sl, span),
        sl::InstrKind::Group(_) => Err(ProseError::new(ProseErrorKind::InvalidGroupTier, span)),
    }
}

// - If instruction

fn prosify_group_if_instr(
    ctx: &Context,
    instr_sl: sl::IfInstr,
    span: Span,
) -> Result<pl::GroupBlock, ProseError> {
    let exp_pl = prosify_exp(ctx, &instr_sl.exp)?;
    let block_pl = prosify_group_block(ctx, instr_sl.block)?;
    let instr_pl = pl::IfInstr {
        exp: exp_pl,
        iter_exps: instr_sl.iter_exps,
        block: block_pl,
        dangle: instr_sl.dangle,
    };
    let instr_kind_pl = pl::InstrKind::If(instr_pl);
    let instr_pl = make_instr(instr_kind_pl, span);
    Ok(vec![instr_pl])
}

// - Hold instruction

fn prosify_group_hold_instr(
    ctx: &Context,
    instr_sl: sl::HoldInstr,
    span: Span,
) -> Result<pl::GroupBlock, ProseError> {
    let hints = ctx
        .hints_rel(&instr_sl.id)
        .map(|hints_rel| annot::Hints {
            prose_true: hints_rel.prose_true.clone(),
            prose_false: hints_rel.prose_false.clone(),
            ..annot::Hints::default()
        })
        .unwrap_or_default();
    validate_hint_alter(&span, &hints, instr_sl.not_exp.args().len())?;
    let not_exp_pl = prosify_not_exp(ctx, &instr_sl.not_exp)?;
    let hold_case_pl = prosify_group_hold_case(ctx, instr_sl.hold_case)?;
    let instr_pl = pl::HoldInstr {
        id: instr_sl.id,
        not_exp: not_exp_pl,
        iter_exps: instr_sl.iter_exps,
        hold_case: hold_case_pl,
    };
    let instr_kind_pl = pl::InstrKind::Hold(instr_pl);
    let instr_pl = make_instr_with_hints(instr_kind_pl, span, hints);
    Ok(vec![instr_pl])
}

fn prosify_group_hold_case(
    ctx: &Context,
    hold_case_sl: sl::HoldCase,
) -> Result<pl::HoldCase<pl::GroupInstr>, ProseError> {
    let hold_case_pl = match hold_case_sl {
        sl::HoldCase::Both(block_hold_sl, block_not_hold_sl) => {
            let block_hold_pl = prosify_group_block(ctx, block_hold_sl)?;
            let block_not_hold_pl = prosify_group_block(ctx, block_not_hold_sl)?;
            pl::HoldCase::Both(block_hold_pl, block_not_hold_pl)
        }
        sl::HoldCase::Hold(block_sl, dangle) => {
            let block_pl = prosify_group_block(ctx, block_sl)?;
            pl::HoldCase::Hold(block_pl, dangle)
        }
        sl::HoldCase::NotHold(block_sl, dangle) => {
            let block_pl = prosify_group_block(ctx, block_sl)?;
            pl::HoldCase::NotHold(block_pl, dangle)
        }
    };
    Ok(hold_case_pl)
}

// - Case instruction

fn prosify_group_case(
    ctx: &Context,
    case_sl: sl::Case,
) -> Result<pl::Case<pl::GroupInstr>, ProseError> {
    let guard_pl = prosify_guard(ctx, &case_sl.guard)?;
    let block_pl = prosify_group_block(ctx, case_sl.block)?;
    Ok(pl::Case { guard: guard_pl, block: block_pl })
}

fn prosify_group_case_instr(
    ctx: &Context,
    instr_sl: sl::CaseInstr,
    span: Span,
) -> Result<pl::GroupBlock, ProseError> {
    let exp_pl = prosify_exp(ctx, &instr_sl.exp)?;
    let mut cases_pl = Vec::with_capacity(instr_sl.cases.len());
    for case_sl in instr_sl.cases {
        let case_pl = prosify_group_case(ctx, case_sl)?;
        cases_pl.push(case_pl);
    }
    let instr_pl = pl::CaseInstr { exp: exp_pl, cases: cases_pl, dangle: instr_sl.dangle };
    let instr_kind_pl = pl::InstrKind::Case(instr_pl);
    let instr_pl = make_instr(instr_kind_pl, span);
    Ok(vec![instr_pl])
}

// - Let instruction

fn prosify_group_let_instr(
    ctx: &Context,
    instr_sl: sl::LetInstr,
    span: Span,
) -> Result<pl::GroupBlock, ProseError> {
    let exp_l_pl = prosify_exp(ctx, &instr_sl.exp_l)?;
    let hints = if matches!(exp_l_pl.node.node, pl::ExpKind::Case(_)) {
        annot::Hints {
            prose_fields: exp_l_pl.hints.prose_fields.clone(),
            ..annot::Hints::default()
        }
    } else {
        annot::Hints::default()
    };
    let exp_r_pl = prosify_exp(ctx, &instr_sl.exp_r)?;
    let instr_pl =
        pl::LetInstr { exp_l: exp_l_pl, exp_r: exp_r_pl, iter_instrs: instr_sl.iter_instrs };
    let instr_kind_pl = pl::InstrKind::Let(instr_pl);
    let instr_pl = make_instr_with_hints(instr_kind_pl, span, hints);
    let block_pl = prosify_group_block(ctx, instr_sl.block)?;
    let mut instrs_pl = vec![instr_pl];
    instrs_pl.extend(block_pl);
    Ok(instrs_pl)
}

// - Debug instruction

fn prosify_group_debug_instr(
    ctx: &Context,
    instr_sl: sl::DebugInstr,
    span: Span,
) -> Result<pl::GroupBlock, ProseError> {
    let exp_pl = prosify_exp(ctx, &instr_sl.exp)?;
    let instr_pl = pl::DebugInstr { exp: exp_pl };
    let instr_kind_pl = pl::InstrKind::Debug(instr_pl);
    let instr_pl = make_instr(instr_kind_pl, span);
    let instrs_follow_pl = prosify_group_instr(ctx, *instr_sl.instr)?;
    let mut instrs_pl = vec![instr_pl];
    instrs_pl.extend(instrs_follow_pl);
    Ok(instrs_pl)
}

// - Rule instruction

fn prosify_group_rule_instr(
    ctx: &Context,
    instr_sl: sl::RuleInstr,
    span: Span,
) -> Result<pl::GroupBlock, ProseError> {
    let hints = ctx
        .hints_rel(&instr_sl.id)
        .map(|hints_rel| annot::Hints {
            prose_in: hints_rel.prose_in.clone(),
            prose_out: hints_rel
                .prose_out
                .as_ref()
                .map(|hint| alter::realign(hint, &instr_sl.input_hint)),
            ..annot::Hints::default()
        })
        .unwrap_or_default();
    let num_args = instr_sl.not_exp.args().len();
    input::validate(&instr_sl.input_hint, num_args)
        .map_err(|error| ProseError::new(ProseErrorKind::Input(error), span.clone()))?;
    let num_inputs = instr_sl.input_hint.indices().len();
    validate_hint_split(&span, &hints, num_inputs, num_args - num_inputs)?;
    let not_exp_pl = prosify_not_exp(ctx, &instr_sl.not_exp)?;
    let instr_pl = pl::RuleInstr {
        id: instr_sl.id,
        not_exp: not_exp_pl,
        input_hint: instr_sl.input_hint,
        iter_instrs: instr_sl.iter_instrs,
    };
    let tier_pl = pl::GroupInstr::Rule(instr_pl);
    let instr_pl = pl::TierInstr { tier: tier_pl };
    let instr_kind_pl = pl::InstrKind::Tier(instr_pl);
    let instr_pl = make_instr_with_hints(instr_kind_pl, span, hints);
    let block_pl = prosify_group_block(ctx, instr_sl.block)?;
    let mut instrs_pl = vec![instr_pl];
    instrs_pl.extend(block_pl);
    Ok(instrs_pl)
}

// - Result instruction

fn prosify_group_result_instr(
    ctx: &Context,
    instr_sl: sl::ResultInstr,
    span: Span,
) -> Result<pl::GroupBlock, ProseError> {
    let hints = ctx
        .hints_rel(ctx.namespace())
        .and_then(|hints_rel| hints_rel.prose_out.as_ref())
        .map(|hint| annot::Hints {
            prose_out: Some(alter::realign(hint, &instr_sl.rel_signature.input_hint)),
            ..annot::Hints::default()
        })
        .unwrap_or_default();
    validate_hint_alter(&span, &hints, instr_sl.exps.len())?;
    let exps_output_pl = prosify_exps(ctx, &instr_sl.exps)?;
    let instr_pl =
        pl::ResultInstr { rel_signature: instr_sl.rel_signature, exps_output: exps_output_pl };
    let tier_pl = pl::GroupInstr::Result(instr_pl);
    let instr_pl = pl::TierInstr { tier: tier_pl };
    let instr_kind_pl = pl::InstrKind::Tier(instr_pl);
    let instr_pl = make_instr_with_hints(instr_kind_pl, span, hints);
    Ok(vec![instr_pl])
}

// - Return instruction

fn prosify_group_return_instr(
    ctx: &Context,
    instr_sl: sl::ReturnInstr,
    span: Span,
) -> Result<pl::GroupBlock, ProseError> {
    let exp_pl = prosify_exp(ctx, &instr_sl.exp)?;
    let instr_pl = pl::ReturnInstr { exp: exp_pl };
    let tier_pl = pl::GroupInstr::Return(instr_pl);
    let instr_pl = pl::TierInstr { tier: tier_pl };
    let instr_kind_pl = pl::InstrKind::Tier(instr_pl);
    let instr_pl = make_instr(instr_kind_pl, span);
    Ok(vec![instr_pl])
}

// - Block

fn prosify_group_block(ctx: &Context, block_sl: sl::Block) -> Result<pl::GroupBlock, ProseError> {
    match block_sl.len() {
        0 => Ok(Vec::new()),
        1 => {
            let instr_sl = block_sl.into_iter().next().unwrap();
            prosify_group_instr(ctx, instr_sl)
        }
        _ => {
            let spans = block_sl
                .iter()
                .map(|instr_sl| instr_sl.span.clone())
                .collect::<Vec<_>>();
            let span = Span::over(&spans);
            let mut blocks_pl = Vec::with_capacity(block_sl.len());
            for instr_sl in block_sl {
                let block_pl = prosify_group_instr(ctx, instr_sl)?;
                blocks_pl.push(block_pl);
            }
            let instr_pl = pl::BacktrackInstr { blocks: blocks_pl };
            let tier_pl = pl::GroupInstr::Backtrack(instr_pl);
            let instr_pl = pl::TierInstr { tier: tier_pl };
            let instr_kind_pl = pl::InstrKind::Tier(instr_pl);
            let instr_pl = make_instr(instr_kind_pl, span);
            Ok(vec![instr_pl])
        }
    }
}

// == Table rows

fn prosify_table_row(ctx: &Context, row_sl: sl::TableRow) -> Result<pl::TableRow, ProseError> {
    let exps_input_pl = prosify_exps(ctx, &row_sl.exps_input)?;
    let exp_pl = prosify_exp(ctx, &row_sl.exp)?;
    let block_pl = prosify_group_block(ctx, row_sl.block)?;
    Ok(pl::TableRow { exps_input: exps_input_pl, exp: exp_pl, block: block_pl })
}

// == Type definitions

// - Type definition

fn prosify_typ_def(typdef_sl: sl::TypDef) -> pl::TypDef {
    match typdef_sl {
        sl::TypDef::Extern(def_typ_sl) => {
            let def_typ_pl = prosify_extern_typ_def(def_typ_sl);
            pl::TypDef::Extern(def_typ_pl)
        }
        sl::TypDef::Defined(def_typ_sl) => {
            let def_typ_pl = prosify_defined_typ_def(*def_typ_sl);
            pl::TypDef::Defined(Box::new(def_typ_pl))
        }
    }
}

// - External type definition

fn prosify_extern_typ_def(def_typ_sl: sl::ExternTyp) -> pl::ExternTyp {
    pl::ExternTyp { id: def_typ_sl.id }
}

// - Defined type definition

fn prosify_defined_typ_def(def_typ_sl: sl::DefinedTyp) -> pl::DefinedTyp {
    pl::DefinedTyp { id: def_typ_sl.id, tparams: def_typ_sl.tparams, def_typ: def_typ_sl.def_typ }
}

// == Meta-variable definitions

fn prosify_var_def(def_var_sl: sl::VarDef) -> pl::VarDef {
    pl::VarDef { id: def_var_sl.id, typ: def_var_sl.typ }
}

// == Relation definitions

// - Relation definition

fn prosify_rel_def(
    ctx: &mut Context,
    def_rel_sl: sl::RelDef,
) -> Result<(pl::RelDef, annot::Hints), ProseError> {
    match def_rel_sl {
        sl::RelDef::Extern(def_rel_sl) => {
            let hints = build_rel_hints(ctx, &def_rel_sl.id, &def_rel_sl.rel_signature)?;
            let def_rel_pl = prosify_extern_rel_def(ctx, def_rel_sl)?;
            Ok((pl::RelDef::Extern(def_rel_pl), hints))
        }
        sl::RelDef::Defined(def_rel_sl) => {
            let hints = build_rel_hints(ctx, &def_rel_sl.id, &def_rel_sl.rel_signature)?;
            let def_rel_pl = prosify_defined_rel_def(ctx, def_rel_sl)?;
            Ok((pl::RelDef::Defined(def_rel_pl), hints))
        }
    }
}

// - External relation definition

fn prosify_extern_rel_def(
    ctx: &Context,
    def_rel_sl: sl::ExternRel,
) -> Result<pl::ExternRel, ProseError> {
    let exps_input_pl = prosify_exps(ctx, &def_rel_sl.exps_input)?;
    Ok(pl::ExternRel {
        id: def_rel_sl.id,
        rel_signature: def_rel_sl.rel_signature,
        exps_input: exps_input_pl,
    })
}

// - Defined relation definition

fn prosify_defined_rel_def(
    ctx: &mut Context,
    def_rel_sl: sl::DefinedRel,
) -> Result<pl::DefinedRel, ProseError> {
    ctx.set_namespace(def_rel_sl.id.clone());
    let exps_input_pl = prosify_exps(ctx, &def_rel_sl.exps_input)?;
    let block_pl = prosify_dispatch_block(ctx, def_rel_sl.block)?;
    let block_else_opt_pl = match def_rel_sl.block_else {
        Some(block_else_sl) => {
            let block_else_pl = prosify_dispatch_block(ctx, block_else_sl)?;
            Some(block_else_pl)
        }
        None => None,
    };
    Ok(pl::DefinedRel {
        id: def_rel_sl.id,
        rel_signature: def_rel_sl.rel_signature,
        exps_input: exps_input_pl,
        block: block_pl,
        block_else_opt: block_else_opt_pl,
    })
}

// == Meta-function definitions

// - Meta-function definition

fn prosify_func_def(
    ctx: &mut Context,
    def_func_sl: sl::MetaFuncDef,
) -> Result<(pl::MetaFuncDef, annot::Hints), ProseError> {
    match def_func_sl {
        sl::MetaFuncDef::Extern(def_func_sl) => {
            let hints = build_func_hints(ctx, &def_func_sl.id);
            let def_func_pl = prosify_extern_func_def(ctx, def_func_sl)?;
            Ok((pl::MetaFuncDef::Extern(def_func_pl), hints))
        }
        sl::MetaFuncDef::Builtin(def_func_sl) => {
            let hints = build_func_hints(ctx, &def_func_sl.id);
            let def_func_pl = prosify_builtin_func_def(ctx, def_func_sl)?;
            Ok((pl::MetaFuncDef::Builtin(def_func_pl), hints))
        }
        sl::MetaFuncDef::Table(def_func_sl) => {
            let hints = build_func_hints(ctx, &def_func_sl.id);
            let def_func_pl = prosify_table_func_def(ctx, def_func_sl)?;
            Ok((pl::MetaFuncDef::Table(def_func_pl), hints))
        }
        sl::MetaFuncDef::Defined(def_func_sl) => {
            let hints = build_func_hints(ctx, &def_func_sl.id);
            let def_func_pl = prosify_defined_func_def(ctx, def_func_sl)?;
            Ok((pl::MetaFuncDef::Defined(def_func_pl), hints))
        }
    }
}

// - External function definition

fn prosify_extern_func_def(
    ctx: &Context,
    def_func_sl: sl::ExternFunc,
) -> Result<pl::ExternFunc, ProseError> {
    let params_pl = prosify_params(ctx, &def_func_sl.params)?;
    Ok(pl::ExternFunc {
        id: def_func_sl.id,
        tparams: def_func_sl.tparams,
        params: params_pl,
        typ: def_func_sl.typ,
    })
}

// - Builtin function definition

fn prosify_builtin_func_def(
    ctx: &Context,
    def_func_sl: sl::BuiltinFunc,
) -> Result<pl::BuiltinFunc, ProseError> {
    let params_pl = prosify_params(ctx, &def_func_sl.params)?;
    Ok(pl::BuiltinFunc {
        id: def_func_sl.id,
        tparams: def_func_sl.tparams,
        params: params_pl,
        typ: def_func_sl.typ,
    })
}

// - Table function definition

fn prosify_table_func_def(
    ctx: &mut Context,
    def_func_sl: sl::TableFunc,
) -> Result<pl::TableFunc, ProseError> {
    ctx.set_namespace(def_func_sl.id.clone());
    let params_pl = prosify_params(ctx, &def_func_sl.params)?;
    let mut rows_pl = Vec::with_capacity(def_func_sl.table_rows.len());
    for row_sl in def_func_sl.table_rows {
        let row_pl = prosify_table_row(ctx, row_sl)?;
        rows_pl.push(row_pl);
    }
    Ok(pl::TableFunc { id: def_func_sl.id, params: params_pl, typ: def_func_sl.typ, rows: rows_pl })
}

// - Defined function definition

fn prosify_defined_func_def(
    ctx: &mut Context,
    def_func_sl: sl::DefinedFunc,
) -> Result<pl::DefinedFunc, ProseError> {
    ctx.set_namespace(def_func_sl.id.clone());
    let params_pl = prosify_params(ctx, &def_func_sl.params)?;
    let block_pl = prosify_group_block(ctx, def_func_sl.block)?;
    let block_else_opt_pl = match def_func_sl.block_else {
        Some(block_else_sl) => {
            let block_else_pl = prosify_group_block(ctx, block_else_sl)?;
            Some(block_else_pl)
        }
        None => None,
    };
    Ok(pl::DefinedFunc {
        id: def_func_sl.id,
        tparams: def_func_sl.tparams,
        params: params_pl,
        typ: def_func_sl.typ,
        block: block_pl,
        block_else_opt: block_else_opt_pl,
    })
}

// == Definitions

// - Definition

fn prosify_def(ctx: &mut Context, def_sl: sl::Def) -> Result<pl::Def, ProseError> {
    let (def_kind_pl, hints) = prosify_def_kind(ctx, def_sl.node)?;
    let def_node_pl = crate::phrase! { node: def_kind_pl, span: def_sl.span };
    let def_pl = crate::annotated! { node: def_node_pl, hints: hints };
    Ok(def_pl)
}

fn prosify_def_kind(
    ctx: &mut Context,
    def_kind_sl: sl::DefKind,
) -> Result<(pl::DefKind, annot::Hints), ProseError> {
    match def_kind_sl {
        sl::DefKind::Typ(def_typ_sl) => {
            let def_typ_pl = prosify_typ_def(def_typ_sl);
            Ok((pl::DefKind::Typ(def_typ_pl), annot::Hints::default()))
        }
        sl::DefKind::Var(def_var_sl) => {
            let def_var_pl = prosify_var_def(def_var_sl);
            Ok((pl::DefKind::Var(def_var_pl), annot::Hints::default()))
        }
        sl::DefKind::Rel(def_rel_sl) => {
            let (def_rel_pl, hints) = prosify_rel_def(ctx, def_rel_sl)?;
            Ok((pl::DefKind::Rel(def_rel_pl), hints))
        }
        sl::DefKind::MetaFunc(def_func_sl) => {
            let (def_func_pl, hints) = prosify_func_def(ctx, def_func_sl)?;
            Ok((pl::DefKind::MetaFunc(def_func_pl), hints))
        }
    }
}

// == Entry point

pub(super) fn prosify(spec_sl: sl::Spec) -> Result<pl::Spec, ProseError> {
    let mut ctx = Context::load(&spec_sl)?;
    let spec_sl = super::expand::spec(spec_sl)?;
    let mut spec_pl = Vec::with_capacity(spec_sl.len());
    for def_sl in spec_sl {
        let def_pl = prosify_def(&mut ctx, def_sl)?;
        spec_pl.push(def_pl);
    }
    let spec_pl = super::shorthand::spec(spec_pl);
    let spec_pl = super::stamp::spec(spec_pl);
    Ok(spec_pl)
}
