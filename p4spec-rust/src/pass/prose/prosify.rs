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

// == Hint lookup

fn hints_of_call_exp(ctx: &Context, id_func: &sl::Id) -> annot::Hints {
    let Some(hints_func) = ctx.hints_func(id_func) else {
        return annot::Hints::default();
    };
    annot::Hints {
        prose_in: hints_func.prose_in.clone(),
        prose_true: hints_func.prose_true.clone(),
        prose_false: hints_func.prose_false.clone(),
        ..annot::Hints::default()
    }
}

fn hints_of_case_exp(ctx: &Context, exp_sl: &sl::Exp, not_exp_sl: &sl::NotExp) -> annot::Hints {
    let il::TypKind::Var(id_typ, _) = exp_sl.note.as_ref() else {
        return annot::Hints::default();
    };
    let Some(hints_case) = ctx.hints_case(id_typ, &not_exp_sl.to_mixop()) else {
        return annot::Hints::default();
    };
    annot::Hints {
        prose: hints_case.prose.clone(),
        prose_fields: hints_case.prose_fields.clone(),
        ..annot::Hints::default()
    }
}

fn hints_of_hold_instr(ctx: &Context, id_rel: &sl::Id) -> annot::Hints {
    let Some(hints_rel) = ctx.hints_rel(id_rel) else {
        return annot::Hints::default();
    };
    annot::Hints {
        prose_true: hints_rel.prose_true.clone(),
        prose_false: hints_rel.prose_false.clone(),
        ..annot::Hints::default()
    }
}

fn hints_of_rule_instr(
    ctx: &Context,
    id_rel: &sl::Id,
    input_hint: &input::InputHint,
) -> annot::Hints {
    let Some(hints_rel) = ctx.hints_rel(id_rel) else {
        return annot::Hints::default();
    };
    annot::Hints {
        prose_in: hints_rel.prose_in.clone(),
        prose_out: hints_rel
            .prose_out
            .as_ref()
            .map(|hint| alter::realign(hint, input_hint)),
        ..annot::Hints::default()
    }
}

fn hints_of_result_instr(ctx: &Context, input_hint: &input::InputHint) -> annot::Hints {
    let Some(hints_rel) = ctx.hints_rel(ctx.namespace()) else {
        return annot::Hints::default();
    };
    annot::Hints {
        prose_out: hints_rel
            .prose_out
            .as_ref()
            .map(|hint| alter::realign(hint, input_hint)),
        ..annot::Hints::default()
    }
}

fn hints_of_group_instr(ctx: &Context) -> annot::Hints {
    let Some(hints_rel) = ctx.hints_rel(ctx.namespace()) else {
        return annot::Hints::default();
    };
    annot::Hints {
        prose_in: hints_rel.prose_in.clone(),
        prose_true: hints_rel.prose_true.clone(),
        ..annot::Hints::default()
    }
}

fn hints_of_func_def(ctx: &Context, id_func: &sl::Id) -> annot::Hints {
    hints_of_call_exp(ctx, id_func)
}

fn hints_of_rel_def(
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

fn hints_of_let_instr(exp_l_pl: &pl::Exp) -> annot::Hints {
    if matches!(exp_l_pl.node.node, pl::ExpKind::Case(_)) {
        annot::Hints {
            prose_fields: exp_l_pl.hints.prose_fields.clone(),
            ..annot::Hints::default()
        }
    } else {
        annot::Hints::default()
    }
}

fn hints_of_def(ctx: &Context, def_kind_sl: &sl::DefKind) -> Result<annot::Hints, ProseError> {
    match def_kind_sl {
        sl::DefKind::Rel(sl::RelDef::Extern(def_rel_sl)) => {
            hints_of_rel_def(ctx, &def_rel_sl.id, &def_rel_sl.rel_signature)
        }
        sl::DefKind::Rel(sl::RelDef::Defined(def_rel_sl)) => {
            hints_of_rel_def(ctx, &def_rel_sl.id, &def_rel_sl.rel_signature)
        }
        sl::DefKind::MetaFunc(sl::MetaFuncDef::Extern(def_func_sl)) => {
            Ok(hints_of_func_def(ctx, &def_func_sl.id))
        }
        sl::DefKind::MetaFunc(sl::MetaFuncDef::Builtin(def_func_sl)) => {
            Ok(hints_of_func_def(ctx, &def_func_sl.id))
        }
        sl::DefKind::MetaFunc(sl::MetaFuncDef::Table(def_func_sl)) => {
            Ok(hints_of_func_def(ctx, &def_func_sl.id))
        }
        sl::DefKind::MetaFunc(sl::MetaFuncDef::Defined(def_func_sl)) => {
            Ok(hints_of_func_def(ctx, &def_func_sl.id))
        }
        sl::DefKind::Typ(_) | sl::DefKind::Var(_) => Ok(annot::Hints::default()),
    }
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
    let exp_kind_pl = prosify_exp_kind(ctx, &exp_sl.node)?;
    let hints = match &exp_sl.node {
        il::ExpKind::Case(not_exp_sl) => {
            let hints = hints_of_case_exp(ctx, exp_sl, not_exp_sl);
            let num_args = not_exp_sl.args().len();
            validate_hint_alter(&exp_sl.span, &hints, num_args)?;
            validate_hint_fields(&exp_sl.span, &hints, num_args)?;
            hints
        }
        il::ExpKind::Call(id, _, args_sl) => {
            let hints = hints_of_call_exp(ctx, id);
            validate_hint_alter(&exp_sl.span, &hints, args_sl.len())?;
            hints
        }
        _ => annot::Hints::default(),
    };
    Ok(crate::annotated! {
        node: crate::note_phrase! {
            node: exp_kind_pl,
            note: exp_sl.note.as_ref().clone(),
            span: exp_sl.span.clone(),
        },
        hints: hints,
    })
}

fn prosify_exp_kind(ctx: &Context, exp_kind_sl: &sl::ExpKind) -> Result<pl::ExpKind, ProseError> {
    let exp_kind_pl = match exp_kind_sl {
        il::ExpKind::Bool(value) => pl::ExpKind::Bool(*value),
        il::ExpKind::Num(num) => pl::ExpKind::Num(num.clone()),
        il::ExpKind::Text(text) => pl::ExpKind::Text(text.clone()),
        il::ExpKind::Var(id) => pl::ExpKind::Var(id.clone()),
        il::ExpKind::Un(op, op_typ, exp_inner_sl) => {
            pl::ExpKind::Un(*op, *op_typ, Box::new(prosify_exp(ctx, exp_inner_sl)?))
        }
        il::ExpKind::Bin(op, op_typ, exp_l_sl, exp_r_sl) => pl::ExpKind::Bin(
            *op,
            *op_typ,
            Box::new(prosify_exp(ctx, exp_l_sl)?),
            Box::new(prosify_exp(ctx, exp_r_sl)?),
        ),
        il::ExpKind::Cmp(op, op_typ, exp_l_sl, exp_r_sl) => pl::ExpKind::Cmp(
            *op,
            *op_typ,
            Box::new(prosify_exp(ctx, exp_l_sl)?),
            Box::new(prosify_exp(ctx, exp_r_sl)?),
        ),
        il::ExpKind::UpCast(typ, exp_inner_sl) => {
            pl::ExpKind::UpCast(typ.as_ref().clone(), Box::new(prosify_exp(ctx, exp_inner_sl)?))
        }
        il::ExpKind::DownCast(typ, exp_inner_sl) => {
            pl::ExpKind::DownCast(typ.as_ref().clone(), Box::new(prosify_exp(ctx, exp_inner_sl)?))
        }
        il::ExpKind::Sub(exp_inner_sl, typ, subcheck) => pl::ExpKind::Sub(
            Box::new(prosify_exp(ctx, exp_inner_sl)?),
            typ.as_ref().clone(),
            subcheck.clone(),
        ),
        il::ExpKind::Match(exp_inner_sl, pattern) => {
            pl::ExpKind::Match(Box::new(prosify_exp(ctx, exp_inner_sl)?), pattern.clone())
        }
        il::ExpKind::Tuple(exps_sl) => pl::ExpKind::Tuple(prosify_exps(ctx, exps_sl)?),
        il::ExpKind::Case(not_exp_sl) => {
            pl::ExpKind::Case(Box::new(prosify_not_exp(ctx, not_exp_sl)?))
        }
        il::ExpKind::Str(fields_sl) => {
            let fields_pl = fields_sl
                .iter()
                .map(|(atom, exp_sl)| Ok((atom.clone(), prosify_exp(ctx, exp_sl)?)))
                .collect::<Result<_, ProseError>>()?;
            pl::ExpKind::Str(fields_pl)
        }
        il::ExpKind::Opt(exp_opt_sl) => pl::ExpKind::Opt(
            exp_opt_sl
                .as_deref()
                .map(|exp_sl| prosify_exp(ctx, exp_sl).map(Box::new))
                .transpose()?,
        ),
        il::ExpKind::List(exps_sl) => pl::ExpKind::List(prosify_exps(ctx, exps_sl)?),
        il::ExpKind::Cons(exp_l_sl, exp_r_sl) => pl::ExpKind::Cons(
            Box::new(prosify_exp(ctx, exp_l_sl)?),
            Box::new(prosify_exp(ctx, exp_r_sl)?),
        ),
        il::ExpKind::Cat(exp_l_sl, exp_r_sl) => pl::ExpKind::Cat(
            Box::new(prosify_exp(ctx, exp_l_sl)?),
            Box::new(prosify_exp(ctx, exp_r_sl)?),
        ),
        il::ExpKind::Mem(exp_l_sl, exp_r_sl) => pl::ExpKind::Mem(
            Box::new(prosify_exp(ctx, exp_l_sl)?),
            Box::new(prosify_exp(ctx, exp_r_sl)?),
        ),
        il::ExpKind::Len(exp_inner_sl) => {
            pl::ExpKind::Len(Box::new(prosify_exp(ctx, exp_inner_sl)?))
        }
        il::ExpKind::Dot(exp_inner_sl, atom) => {
            pl::ExpKind::Dot(Box::new(prosify_exp(ctx, exp_inner_sl)?), atom.clone())
        }
        il::ExpKind::Idx(exp_l_sl, exp_r_sl) => pl::ExpKind::Idx(
            Box::new(prosify_exp(ctx, exp_l_sl)?),
            Box::new(prosify_exp(ctx, exp_r_sl)?),
        ),
        il::ExpKind::Slice(exp_base_sl, exp_idx_sl, exp_len_sl) => pl::ExpKind::Slice(
            Box::new(prosify_exp(ctx, exp_base_sl)?),
            Box::new(prosify_exp(ctx, exp_idx_sl)?),
            Box::new(prosify_exp(ctx, exp_len_sl)?),
        ),
        il::ExpKind::Upd(exp_base_sl, path_sl, exp_field_sl) => pl::ExpKind::Upd(
            Box::new(prosify_exp(ctx, exp_base_sl)?),
            Box::new(prosify_path(ctx, path_sl)?),
            Box::new(prosify_exp(ctx, exp_field_sl)?),
        ),
        il::ExpKind::Call(id, targs, args_sl) => pl::ExpKind::Call(
            id.clone(),
            targs.clone(),
            args_sl
                .iter()
                .map(|arg_sl| prosify_arg(ctx, arg_sl))
                .collect::<Result<_, _>>()?,
        ),
        il::ExpKind::Iter(exp_inner_sl, iter_exp) => {
            pl::ExpKind::Iter(Box::new(prosify_exp(ctx, exp_inner_sl)?), iter_exp.clone())
        }
    };
    Ok(exp_kind_pl)
}

// - Expression list

fn prosify_exps(ctx: &Context, exps_sl: &[sl::Exp]) -> Result<Vec<pl::Exp>, ProseError> {
    exps_sl
        .iter()
        .map(|exp_sl| prosify_exp(ctx, exp_sl))
        .collect()
}

// - Notation expression

fn prosify_not_exp(ctx: &Context, not_exp_sl: &sl::NotExp) -> Result<pl::NotExp, ProseError> {
    Ok(match not_exp_sl {
        Mixfix::Arg(exp_sl) => Mixfix::Arg(prosify_exp(ctx, exp_sl)?),
        Mixfix::Atom(atom) => Mixfix::Atom(atom.clone()),
        Mixfix::Brack(atom_l, not_exp_inner_sl, atom_r) => Mixfix::Brack(
            atom_l.clone(),
            Box::new(prosify_not_exp(ctx, not_exp_inner_sl)?),
            atom_r.clone(),
        ),
        Mixfix::Infix(not_exp_l_sl, atom, not_exp_r_sl) => Mixfix::Infix(
            Box::new(prosify_not_exp(ctx, not_exp_l_sl)?),
            atom.clone(),
            Box::new(prosify_not_exp(ctx, not_exp_r_sl)?),
        ),
        Mixfix::Seq(not_exps_sl) => Mixfix::Seq(
            not_exps_sl
                .iter()
                .map(|not_exp_sl| prosify_not_exp(ctx, not_exp_sl))
                .collect::<Result<_, _>>()?,
        ),
    })
}

// == Paths

fn prosify_path(ctx: &Context, path_sl: &sl::Path) -> Result<pl::Path, ProseError> {
    let path_kind_pl = prosify_path_kind(ctx, &path_sl.node)?;
    Ok(crate::note_phrase! {
        node: path_kind_pl,
        note: path_sl.note.as_ref().clone(),
        span: path_sl.span.clone(),
    })
}

fn prosify_path_kind(
    ctx: &Context,
    path_kind_sl: &sl::PathKind,
) -> Result<pl::PathKind, ProseError> {
    let path_kind_pl = match path_kind_sl {
        il::PathKind::Root => pl::PathKind::Root,
        il::PathKind::Idx(path_inner_sl, exp_idx_sl) => pl::PathKind::Idx(
            Box::new(prosify_path(ctx, path_inner_sl)?),
            Box::new(prosify_exp(ctx, exp_idx_sl)?),
        ),
        il::PathKind::Slice(path_inner_sl, exp_idx_sl, exp_len_sl) => pl::PathKind::Slice(
            Box::new(prosify_path(ctx, path_inner_sl)?),
            Box::new(prosify_exp(ctx, exp_idx_sl)?),
            Box::new(prosify_exp(ctx, exp_len_sl)?),
        ),
        il::PathKind::Dot(path_inner_sl, atom) => {
            pl::PathKind::Dot(Box::new(prosify_path(ctx, path_inner_sl)?), atom.clone())
        }
    };
    Ok(path_kind_pl)
}

// == Arguments

fn prosify_arg(ctx: &Context, arg_sl: &sl::Arg) -> Result<pl::Arg, ProseError> {
    let arg_kind_pl = prosify_arg_kind(ctx, &arg_sl.node)?;
    Ok(crate::phrase! { node: arg_kind_pl, span: arg_sl.span.clone() })
}

fn prosify_arg_kind(ctx: &Context, arg_kind_sl: &sl::ArgKind) -> Result<pl::ArgKind, ProseError> {
    let arg_kind_pl = match arg_kind_sl {
        il::ArgKind::Exp(exp_sl) => pl::ArgKind::Exp(Box::new(prosify_exp(ctx, exp_sl)?)),
        il::ArgKind::Def(id) => pl::ArgKind::Def(id.clone()),
    };
    Ok(arg_kind_pl)
}

// == Parameters

fn prosify_param(ctx: &Context, param_sl: &sl::Param) -> Result<pl::Param, ProseError> {
    let param_kind_pl = prosify_param_kind(ctx, &param_sl.node)?;
    Ok(crate::phrase! { node: param_kind_pl, span: param_sl.span.clone() })
}

fn prosify_param_kind(
    ctx: &Context,
    param_kind_sl: &sl::ParamKind,
) -> Result<pl::ParamKind, ProseError> {
    let param_kind_pl = match param_kind_sl {
        sl::ParamKind::Exp(typ, exp_sl) => {
            pl::ParamKind::Exp(typ.clone(), Box::new(prosify_exp(ctx, exp_sl)?))
        }
        sl::ParamKind::Def(id, tparams, params_sl, typ) => pl::ParamKind::Def(
            id.clone(),
            tparams.clone(),
            params_sl
                .iter()
                .map(|param_sl| prosify_param(ctx, param_sl))
                .collect::<Result<_, _>>()?,
            typ.clone(),
        ),
    };
    Ok(param_kind_pl)
}

fn prosify_params(ctx: &Context, params_sl: &[sl::Param]) -> Result<Vec<pl::Param>, ProseError> {
    params_sl
        .iter()
        .map(|param_sl| prosify_param(ctx, param_sl))
        .collect()
}

// == Guards

fn prosify_guard(ctx: &Context, guard_sl: &sl::Guard) -> Result<pl::Guard, ProseError> {
    Ok(match guard_sl {
        sl::Guard::Bool(value) => pl::Guard::Bool(*value),
        sl::Guard::Cmp(op, op_typ, exp_sl) => {
            pl::Guard::Cmp(*op, *op_typ, prosify_exp(ctx, exp_sl)?)
        }
        sl::Guard::Sub(typ, subcheck) => pl::Guard::Sub(typ.clone(), subcheck.clone()),
        sl::Guard::Match(pattern) => pl::Guard::Match(pattern.clone()),
        sl::Guard::Mem(exp_sl) => pl::Guard::Mem(prosify_exp(ctx, exp_sl)?),
    })
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
    crate::annotated! {
        node: crate::note_phrase! {
            node: instr_kind_pl,
            note: None,
            span: span,
        },
        hints: hints,
    }
}

// == Dispatch instructions

// - Instruction

fn prosify_instr_dispatch(
    ctx: &Context,
    instr_sl: sl::Instr,
) -> Result<pl::BlockDispatch, ProseError> {
    prosify_instr_kind_dispatch(ctx, instr_sl.node, instr_sl.span)
}

fn prosify_instr_kind_dispatch(
    ctx: &Context,
    instr_kind_sl: sl::InstrKind,
    span: Span,
) -> Result<pl::BlockDispatch, ProseError> {
    match instr_kind_sl {
        sl::InstrKind::If(instr_sl) => prosify_if_instr_dispatch(ctx, instr_sl, span),
        sl::InstrKind::Hold(instr_sl) => prosify_hold_instr_dispatch(ctx, instr_sl, span),
        sl::InstrKind::Case(instr_sl) => prosify_case_instr_dispatch(ctx, instr_sl, span),
        sl::InstrKind::Let(instr_sl) => prosify_let_instr_dispatch(ctx, instr_sl, span),
        sl::InstrKind::Debug(instr_sl) => prosify_debug_instr_dispatch(ctx, instr_sl, span),
        sl::InstrKind::Group(instr_sl) => prosify_group_instr_dispatch(ctx, instr_sl, span),
        sl::InstrKind::Rule(_) | sl::InstrKind::Result(_) | sl::InstrKind::Return(_) => {
            Err(ProseError::new(ProseErrorKind::InvalidDispatchTier, span))
        }
    }
}

// - If instruction

fn prosify_if_instr_dispatch(
    ctx: &Context,
    instr_sl: sl::IfInstr,
    span: Span,
) -> Result<pl::BlockDispatch, ProseError> {
    let instr_kind_pl = pl::InstrKind::If(pl::IfInstr {
        exp: prosify_exp(ctx, &instr_sl.exp)?,
        iter_exps: instr_sl.iter_exps,
        block: prosify_block_dispatch(ctx, instr_sl.block)?,
        dangle: instr_sl.dangle,
    });
    Ok(vec![make_instr(instr_kind_pl, span)])
}

// - Hold instruction

fn prosify_hold_instr_dispatch(
    ctx: &Context,
    instr_sl: sl::HoldInstr,
    span: Span,
) -> Result<pl::BlockDispatch, ProseError> {
    let hints = hints_of_hold_instr(ctx, &instr_sl.id);
    validate_hint_alter(&span, &hints, instr_sl.not_exp.args().len())?;
    let instr_kind_pl = pl::InstrKind::Hold(pl::HoldInstr {
        id: instr_sl.id,
        not_exp: prosify_not_exp(ctx, &instr_sl.not_exp)?,
        iter_exps: instr_sl.iter_exps,
        hold_case: prosify_hold_case_dispatch(ctx, instr_sl.hold_case)?,
    });
    Ok(vec![make_instr_with_hints(instr_kind_pl, span, hints)])
}

fn prosify_hold_case_dispatch(
    ctx: &Context,
    hold_case_sl: sl::HoldCase,
) -> Result<pl::HoldCase<pl::InstrDispatch>, ProseError> {
    Ok(match hold_case_sl {
        sl::HoldCase::Both(block_hold_sl, block_not_hold_sl) => pl::HoldCase::Both(
            prosify_block_dispatch(ctx, block_hold_sl)?,
            prosify_block_dispatch(ctx, block_not_hold_sl)?,
        ),
        sl::HoldCase::Hold(block_sl, dangle) => {
            pl::HoldCase::Hold(prosify_block_dispatch(ctx, block_sl)?, dangle)
        }
        sl::HoldCase::NotHold(block_sl, dangle) => {
            pl::HoldCase::NotHold(prosify_block_dispatch(ctx, block_sl)?, dangle)
        }
    })
}

// - Case instruction

fn prosify_case_instr_dispatch(
    ctx: &Context,
    instr_sl: sl::CaseInstr,
    span: Span,
) -> Result<pl::BlockDispatch, ProseError> {
    let cases_pl = instr_sl
        .cases
        .into_iter()
        .map(|case_sl| {
            Ok(pl::Case {
                guard: prosify_guard(ctx, &case_sl.guard)?,
                block: prosify_block_dispatch(ctx, case_sl.block)?,
            })
        })
        .collect::<Result<_, ProseError>>()?;
    let instr_kind_pl = pl::InstrKind::Case(pl::CaseInstr {
        exp: prosify_exp(ctx, &instr_sl.exp)?,
        cases: cases_pl,
        dangle: instr_sl.dangle,
    });
    Ok(vec![make_instr(instr_kind_pl, span)])
}

// - Let instruction

fn prosify_let_instr_dispatch(
    ctx: &Context,
    instr_sl: sl::LetInstr,
    span: Span,
) -> Result<pl::BlockDispatch, ProseError> {
    let exp_l_pl = prosify_exp(ctx, &instr_sl.exp_l)?;
    let hints = hints_of_let_instr(&exp_l_pl);
    let instr_kind_pl = pl::InstrKind::Let(pl::LetInstr {
        exp_l: exp_l_pl,
        exp_r: prosify_exp(ctx, &instr_sl.exp_r)?,
        iter_instrs: instr_sl.iter_instrs,
    });
    let mut instrs_pl = vec![make_instr_with_hints(instr_kind_pl, span, hints)];
    instrs_pl.extend(prosify_block_dispatch(ctx, instr_sl.block)?);
    Ok(instrs_pl)
}

// - Debug instruction

fn prosify_debug_instr_dispatch(
    ctx: &Context,
    instr_sl: sl::DebugInstr,
    span: Span,
) -> Result<pl::BlockDispatch, ProseError> {
    let instr_kind_pl =
        pl::InstrKind::Debug(pl::DebugInstr { exp: prosify_exp(ctx, &instr_sl.exp)? });
    let mut instrs_pl = vec![make_instr(instr_kind_pl, span)];
    instrs_pl.extend(prosify_instr_dispatch(ctx, *instr_sl.instr)?);
    Ok(instrs_pl)
}

// - Group instruction

fn prosify_group_instr_dispatch(
    ctx: &Context,
    instr_sl: sl::GroupInstr,
    span: Span,
) -> Result<pl::BlockDispatch, ProseError> {
    let hints = hints_of_group_instr(ctx);
    input::validate(&instr_sl.rel_signature.input_hint, instr_sl.exps.len())
        .map_err(|error| ProseError::new(ProseErrorKind::Input(error), span.clone()))?;
    validate_hint_alter(&span, &hints, instr_sl.rel_signature.input_hint.indices().len())?;
    let instr_kind_pl = pl::InstrKind::Tier(pl::TierInstr {
        tier: pl::InstrDispatch::Group(pl::GroupDispatchInstr {
            id_rel: ctx.namespace().clone(),
            id_group: instr_sl.id,
            rel_signature: instr_sl.rel_signature,
            exps_input: prosify_exps(ctx, &instr_sl.exps)?,
            block: prosify_block_group(ctx, instr_sl.block)?,
        }),
    });
    Ok(vec![make_instr_with_hints(instr_kind_pl, span, hints)])
}

// - Block

fn prosify_block_dispatch(
    ctx: &Context,
    block_sl: sl::Block,
) -> Result<pl::BlockDispatch, ProseError> {
    match block_sl.len() {
        0 => Ok(Vec::new()),
        1 => prosify_instr_dispatch(ctx, block_sl.into_iter().next().unwrap()),
        _ => {
            let span = Span::over(
                &block_sl
                    .iter()
                    .map(|instr_sl| instr_sl.span.clone())
                    .collect::<Vec<_>>(),
            );
            let blocks_pl = block_sl
                .into_iter()
                .map(|instr_sl| prosify_instr_dispatch(ctx, instr_sl))
                .collect::<Result<_, _>>()?;
            let instr_kind_pl = pl::InstrKind::Tier(pl::TierInstr {
                tier: pl::InstrDispatch::Route(pl::RouteDispatchInstr { blocks: blocks_pl }),
            });
            Ok(vec![make_instr(instr_kind_pl, span)])
        }
    }
}

// == Group instructions

// - Instruction

fn prosify_instr_group(ctx: &Context, instr_sl: sl::Instr) -> Result<pl::BlockGroup, ProseError> {
    prosify_instr_kind_group(ctx, instr_sl.node, instr_sl.span)
}

fn prosify_instr_kind_group(
    ctx: &Context,
    instr_kind_sl: sl::InstrKind,
    span: Span,
) -> Result<pl::BlockGroup, ProseError> {
    match instr_kind_sl {
        sl::InstrKind::If(instr_sl) => prosify_if_instr_group(ctx, instr_sl, span),
        sl::InstrKind::Hold(instr_sl) => prosify_hold_instr_group(ctx, instr_sl, span),
        sl::InstrKind::Case(instr_sl) => prosify_case_instr_group(ctx, instr_sl, span),
        sl::InstrKind::Let(instr_sl) => prosify_let_instr_group(ctx, instr_sl, span),
        sl::InstrKind::Debug(instr_sl) => prosify_debug_instr_group(ctx, instr_sl, span),
        sl::InstrKind::Rule(instr_sl) => prosify_rule_instr_group(ctx, instr_sl, span),
        sl::InstrKind::Result(instr_sl) => prosify_result_instr_group(ctx, instr_sl, span),
        sl::InstrKind::Return(instr_sl) => prosify_return_instr_group(ctx, instr_sl, span),
        sl::InstrKind::Group(_) => Err(ProseError::new(ProseErrorKind::InvalidGroupTier, span)),
    }
}

// - If instruction

fn prosify_if_instr_group(
    ctx: &Context,
    instr_sl: sl::IfInstr,
    span: Span,
) -> Result<pl::BlockGroup, ProseError> {
    let instr_kind_pl = pl::InstrKind::If(pl::IfInstr {
        exp: prosify_exp(ctx, &instr_sl.exp)?,
        iter_exps: instr_sl.iter_exps,
        block: prosify_block_group(ctx, instr_sl.block)?,
        dangle: instr_sl.dangle,
    });
    Ok(vec![make_instr(instr_kind_pl, span)])
}

// - Hold instruction

fn prosify_hold_instr_group(
    ctx: &Context,
    instr_sl: sl::HoldInstr,
    span: Span,
) -> Result<pl::BlockGroup, ProseError> {
    let hints = hints_of_hold_instr(ctx, &instr_sl.id);
    validate_hint_alter(&span, &hints, instr_sl.not_exp.args().len())?;
    let instr_kind_pl = pl::InstrKind::Hold(pl::HoldInstr {
        id: instr_sl.id,
        not_exp: prosify_not_exp(ctx, &instr_sl.not_exp)?,
        iter_exps: instr_sl.iter_exps,
        hold_case: prosify_hold_case_group(ctx, instr_sl.hold_case)?,
    });
    Ok(vec![make_instr_with_hints(instr_kind_pl, span, hints)])
}

fn prosify_hold_case_group(
    ctx: &Context,
    hold_case_sl: sl::HoldCase,
) -> Result<pl::HoldCase<pl::InstrGroup>, ProseError> {
    Ok(match hold_case_sl {
        sl::HoldCase::Both(block_hold_sl, block_not_hold_sl) => pl::HoldCase::Both(
            prosify_block_group(ctx, block_hold_sl)?,
            prosify_block_group(ctx, block_not_hold_sl)?,
        ),
        sl::HoldCase::Hold(block_sl, dangle) => {
            pl::HoldCase::Hold(prosify_block_group(ctx, block_sl)?, dangle)
        }
        sl::HoldCase::NotHold(block_sl, dangle) => {
            pl::HoldCase::NotHold(prosify_block_group(ctx, block_sl)?, dangle)
        }
    })
}

// - Case instruction

fn prosify_case_instr_group(
    ctx: &Context,
    instr_sl: sl::CaseInstr,
    span: Span,
) -> Result<pl::BlockGroup, ProseError> {
    let cases_pl = instr_sl
        .cases
        .into_iter()
        .map(|case_sl| {
            Ok(pl::Case {
                guard: prosify_guard(ctx, &case_sl.guard)?,
                block: prosify_block_group(ctx, case_sl.block)?,
            })
        })
        .collect::<Result<_, ProseError>>()?;
    let instr_kind_pl = pl::InstrKind::Case(pl::CaseInstr {
        exp: prosify_exp(ctx, &instr_sl.exp)?,
        cases: cases_pl,
        dangle: instr_sl.dangle,
    });
    Ok(vec![make_instr(instr_kind_pl, span)])
}

// - Let instruction

fn prosify_let_instr_group(
    ctx: &Context,
    instr_sl: sl::LetInstr,
    span: Span,
) -> Result<pl::BlockGroup, ProseError> {
    let exp_l_pl = prosify_exp(ctx, &instr_sl.exp_l)?;
    let hints = hints_of_let_instr(&exp_l_pl);
    let instr_kind_pl = pl::InstrKind::Let(pl::LetInstr {
        exp_l: exp_l_pl,
        exp_r: prosify_exp(ctx, &instr_sl.exp_r)?,
        iter_instrs: instr_sl.iter_instrs,
    });
    let mut instrs_pl = vec![make_instr_with_hints(instr_kind_pl, span, hints)];
    instrs_pl.extend(prosify_block_group(ctx, instr_sl.block)?);
    Ok(instrs_pl)
}

// - Debug instruction

fn prosify_debug_instr_group(
    ctx: &Context,
    instr_sl: sl::DebugInstr,
    span: Span,
) -> Result<pl::BlockGroup, ProseError> {
    let instr_kind_pl =
        pl::InstrKind::Debug(pl::DebugInstr { exp: prosify_exp(ctx, &instr_sl.exp)? });
    let mut instrs_pl = vec![make_instr(instr_kind_pl, span)];
    instrs_pl.extend(prosify_instr_group(ctx, *instr_sl.instr)?);
    Ok(instrs_pl)
}

// - Rule instruction

fn prosify_rule_instr_group(
    ctx: &Context,
    instr_sl: sl::RuleInstr,
    span: Span,
) -> Result<pl::BlockGroup, ProseError> {
    let hints = hints_of_rule_instr(ctx, &instr_sl.id, &instr_sl.input_hint);
    let num_args = instr_sl.not_exp.args().len();
    input::validate(&instr_sl.input_hint, num_args)
        .map_err(|error| ProseError::new(ProseErrorKind::Input(error), span.clone()))?;
    let num_inputs = instr_sl.input_hint.indices().len();
    validate_hint_split(&span, &hints, num_inputs, num_args - num_inputs)?;
    let instr_kind_pl = pl::InstrKind::Tier(pl::TierInstr {
        tier: pl::InstrGroup::Rule(pl::RuleGroupInstr {
            id: instr_sl.id,
            not_exp: prosify_not_exp(ctx, &instr_sl.not_exp)?,
            input_hint: instr_sl.input_hint,
            iter_instrs: instr_sl.iter_instrs,
        }),
    });
    let mut instrs_pl = vec![make_instr_with_hints(instr_kind_pl, span, hints)];
    instrs_pl.extend(prosify_block_group(ctx, instr_sl.block)?);
    Ok(instrs_pl)
}

// - Result instruction

fn prosify_result_instr_group(
    ctx: &Context,
    instr_sl: sl::ResultInstr,
    span: Span,
) -> Result<pl::BlockGroup, ProseError> {
    let hints = hints_of_result_instr(ctx, &instr_sl.rel_signature.input_hint);
    validate_hint_alter(&span, &hints, instr_sl.exps.len())?;
    let instr_kind_pl = pl::InstrKind::Tier(pl::TierInstr {
        tier: pl::InstrGroup::Result(pl::ResultGroupInstr {
            rel_signature: instr_sl.rel_signature,
            exps_output: prosify_exps(ctx, &instr_sl.exps)?,
        }),
    });
    Ok(vec![make_instr_with_hints(instr_kind_pl, span, hints)])
}

// - Return instruction

fn prosify_return_instr_group(
    ctx: &Context,
    instr_sl: sl::ReturnInstr,
    span: Span,
) -> Result<pl::BlockGroup, ProseError> {
    let instr_kind_pl = pl::InstrKind::Tier(pl::TierInstr {
        tier: pl::InstrGroup::Return(pl::ReturnGroupInstr {
            exp: prosify_exp(ctx, &instr_sl.exp)?,
        }),
    });
    Ok(vec![make_instr(instr_kind_pl, span)])
}

// - Block

fn prosify_block_group(ctx: &Context, block_sl: sl::Block) -> Result<pl::BlockGroup, ProseError> {
    match block_sl.len() {
        0 => Ok(Vec::new()),
        1 => prosify_instr_group(ctx, block_sl.into_iter().next().unwrap()),
        _ => {
            let span = Span::over(
                &block_sl
                    .iter()
                    .map(|instr_sl| instr_sl.span.clone())
                    .collect::<Vec<_>>(),
            );
            let blocks_pl = block_sl
                .into_iter()
                .map(|instr_sl| prosify_instr_group(ctx, instr_sl))
                .collect::<Result<_, _>>()?;
            let instr_kind_pl = pl::InstrKind::Tier(pl::TierInstr {
                tier: pl::InstrGroup::Backtrack(pl::BacktrackGroupInstr { blocks: blocks_pl }),
            });
            Ok(vec![make_instr(instr_kind_pl, span)])
        }
    }
}

// == Table rows

fn prosify_table_row(ctx: &Context, row_sl: sl::TableRow) -> Result<pl::TableRow, ProseError> {
    Ok(pl::TableRow {
        exps_input: prosify_exps(ctx, &row_sl.exps_input)?,
        exp: prosify_exp(ctx, &row_sl.exp)?,
        block: prosify_block_group(ctx, row_sl.block)?,
    })
}

// == Type definitions

// - Type definition

fn prosify_typ_def(typdef_sl: sl::TypDef) -> pl::TypDef {
    match typdef_sl {
        sl::TypDef::Extern(def_typ_sl) => pl::TypDef::Extern(prosify_extern_typ_def(def_typ_sl)),
        sl::TypDef::Defined(def_typ_sl) => {
            pl::TypDef::Defined(Box::new(prosify_defined_typ_def(*def_typ_sl)))
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

fn prosify_rel_def(ctx: &mut Context, def_rel_sl: sl::RelDef) -> Result<pl::RelDef, ProseError> {
    match def_rel_sl {
        sl::RelDef::Extern(def_rel_sl) => {
            Ok(pl::RelDef::Extern(prosify_extern_rel_def(ctx, def_rel_sl)?))
        }
        sl::RelDef::Defined(def_rel_sl) => {
            Ok(pl::RelDef::Defined(prosify_defined_rel_def(ctx, def_rel_sl)?))
        }
    }
}

// - External relation definition

fn prosify_extern_rel_def(
    ctx: &Context,
    def_rel_sl: sl::ExternRel,
) -> Result<pl::ExternRel, ProseError> {
    Ok(pl::ExternRel {
        id: def_rel_sl.id,
        rel_signature: def_rel_sl.rel_signature,
        exps_input: prosify_exps(ctx, &def_rel_sl.exps_input)?,
    })
}

// - Defined relation definition

fn prosify_defined_rel_def(
    ctx: &mut Context,
    def_rel_sl: sl::DefinedRel,
) -> Result<pl::DefinedRel, ProseError> {
    ctx.set_namespace(def_rel_sl.id.clone());
    Ok(pl::DefinedRel {
        id: def_rel_sl.id,
        rel_signature: def_rel_sl.rel_signature,
        exps_input: prosify_exps(ctx, &def_rel_sl.exps_input)?,
        block: prosify_block_dispatch(ctx, def_rel_sl.block)?,
        block_else_opt: def_rel_sl
            .block_else
            .map(|block_sl| prosify_block_dispatch(ctx, block_sl))
            .transpose()?,
    })
}

// == Meta-function definitions

// - Meta-function definition

fn prosify_func_def(
    ctx: &mut Context,
    def_func_sl: sl::MetaFuncDef,
) -> Result<pl::MetaFuncDef, ProseError> {
    match def_func_sl {
        sl::MetaFuncDef::Extern(def_func_sl) => {
            Ok(pl::MetaFuncDef::Extern(prosify_extern_func_def(ctx, def_func_sl)?))
        }
        sl::MetaFuncDef::Builtin(def_func_sl) => {
            Ok(pl::MetaFuncDef::Builtin(prosify_builtin_func_def(ctx, def_func_sl)?))
        }
        sl::MetaFuncDef::Table(def_func_sl) => {
            Ok(pl::MetaFuncDef::Table(prosify_table_func_def(ctx, def_func_sl)?))
        }
        sl::MetaFuncDef::Defined(def_func_sl) => {
            Ok(pl::MetaFuncDef::Defined(prosify_defined_func_def(ctx, def_func_sl)?))
        }
    }
}

// - External function definition

fn prosify_extern_func_def(
    ctx: &Context,
    def_func_sl: sl::ExternFunc,
) -> Result<pl::ExternFunc, ProseError> {
    ctx.validate_tparams(&def_func_sl.tparams)?;
    Ok(pl::ExternFunc {
        id: def_func_sl.id,
        tparams: def_func_sl.tparams,
        params: prosify_params(ctx, &def_func_sl.params)?,
        typ: def_func_sl.typ,
    })
}

// - Builtin function definition

fn prosify_builtin_func_def(
    ctx: &Context,
    def_func_sl: sl::BuiltinFunc,
) -> Result<pl::BuiltinFunc, ProseError> {
    ctx.validate_tparams(&def_func_sl.tparams)?;
    Ok(pl::BuiltinFunc {
        id: def_func_sl.id,
        tparams: def_func_sl.tparams,
        params: prosify_params(ctx, &def_func_sl.params)?,
        typ: def_func_sl.typ,
    })
}

// - Table function definition

fn prosify_table_func_def(
    ctx: &mut Context,
    def_func_sl: sl::TableFunc,
) -> Result<pl::TableFunc, ProseError> {
    ctx.set_namespace(def_func_sl.id.clone());
    Ok(pl::TableFunc {
        id: def_func_sl.id,
        params: prosify_params(ctx, &def_func_sl.params)?,
        typ: def_func_sl.typ,
        rows: def_func_sl
            .table_rows
            .into_iter()
            .map(|row_sl| prosify_table_row(ctx, row_sl))
            .collect::<Result<_, _>>()?,
    })
}

// - Defined function definition

fn prosify_defined_func_def(
    ctx: &mut Context,
    def_func_sl: sl::DefinedFunc,
) -> Result<pl::DefinedFunc, ProseError> {
    ctx.validate_tparams(&def_func_sl.tparams)?;
    ctx.set_namespace(def_func_sl.id.clone());
    Ok(pl::DefinedFunc {
        id: def_func_sl.id,
        tparams: def_func_sl.tparams,
        params: prosify_params(ctx, &def_func_sl.params)?,
        typ: def_func_sl.typ,
        block: prosify_block_group(ctx, def_func_sl.block)?,
        block_else_opt: def_func_sl
            .block_else
            .map(|block_sl| prosify_block_group(ctx, block_sl))
            .transpose()?,
    })
}

// == Definitions

// - Definition

fn prosify_def(ctx: &mut Context, def_sl: sl::Def) -> Result<pl::Def, ProseError> {
    let hints = hints_of_def(ctx, &def_sl.node)?;
    let def_kind_pl = prosify_def_kind(ctx, def_sl.node)?;
    Ok(crate::annotated! {
        node: crate::phrase! { node: def_kind_pl, span: def_sl.span },
        hints: hints,
    })
}

fn prosify_def_kind(
    ctx: &mut Context,
    def_kind_sl: sl::DefKind,
) -> Result<pl::DefKind, ProseError> {
    match def_kind_sl {
        sl::DefKind::Typ(typdef_sl) => Ok(pl::DefKind::Typ(prosify_typ_def(typdef_sl))),
        sl::DefKind::Var(def_var_sl) => Ok(pl::DefKind::Var(prosify_var_def(def_var_sl))),
        sl::DefKind::Rel(def_rel_sl) => Ok(pl::DefKind::Rel(prosify_rel_def(ctx, def_rel_sl)?)),
        sl::DefKind::MetaFunc(def_func_sl) => {
            Ok(pl::DefKind::MetaFunc(prosify_func_def(ctx, def_func_sl)?))
        }
    }
}

// == Entry point

pub(super) fn prosify(spec_sl: sl::Spec) -> Result<pl::Spec, ProseError> {
    let mut ctx = Context::load(&spec_sl)?;
    let mut spec_pl = Vec::with_capacity(spec_sl.len());
    for def_sl in super::expand::spec(spec_sl)? {
        spec_pl.push(prosify_def(&mut ctx, def_sl)?);
    }
    Ok(super::stamp::spec(super::shorthand::spec(spec_pl)))
}
