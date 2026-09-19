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

fn validate_alterations(
    span: &Span,
    hints: &annot::Hints,
    item_count: usize,
) -> Result<(), ProseError> {
    for hint in
        [&hints.prose, &hints.prose_in, &hints.prose_out, &hints.prose_true, &hints.prose_false]
            .into_iter()
            .flatten()
    {
        alter::validate_count(hint, item_count)
            .map_err(|error| ProseError::new(ProseErrorKind::Alteration(error), span.clone()))?;
    }
    Ok(())
}

fn call_hints(ctx: &Context, id: &sl::Id) -> annot::Hints {
    let hints_func = ctx.func_hints(id);
    annot::Hints {
        prose_in: hints_func.prose_in,
        prose_true: hints_func.prose_true,
        prose_false: hints_func.prose_false,
        ..annot::Hints::default()
    }
}

fn validate_fields(span: &Span, hints: &annot::Hints, arity: usize) -> Result<(), ProseError> {
    if let Some(hint) = &hints.prose_fields {
        fields::validate(hint, arity)
            .map_err(|error| ProseError::new(ProseErrorKind::Field(error), span.clone()))?;
    }
    Ok(())
}

fn validate_split(
    span: &Span,
    hints: &annot::Hints,
    input_count: usize,
    output_count: usize,
) -> Result<(), ProseError> {
    if let Some(hint) = &hints.prose_in {
        alter::validate_count(hint, input_count)
            .map_err(|error| ProseError::new(ProseErrorKind::Alteration(error), span.clone()))?;
    }
    if let Some(hint) = &hints.prose_out {
        alter::validate_count(hint, output_count)
            .map_err(|error| ProseError::new(ProseErrorKind::Alteration(error), span.clone()))?;
    }
    Ok(())
}

fn case_hints(ctx: &Context, exp_sl: &sl::Exp, not_exp_sl: &sl::NotExp) -> annot::Hints {
    let il::TypKind::Var(id_typ, _) = exp_sl.note.as_ref() else {
        return annot::Hints::default();
    };
    let hints_case = ctx.case_hints(id_typ, &not_exp_sl.to_mixop());
    annot::Hints {
        prose: hints_case.prose,
        prose_fields: hints_case.prose_fields,
        ..annot::Hints::default()
    }
}

fn hold_hints(ctx: &Context, id: &sl::Id) -> annot::Hints {
    let hints_rel = ctx.rel_hints(id);
    annot::Hints {
        prose_true: hints_rel.prose_true,
        prose_false: hints_rel.prose_false,
        ..annot::Hints::default()
    }
}

fn rule_hints(ctx: &Context, id: &sl::Id, input_hint: &input::InputHint) -> annot::Hints {
    let hints_rel = ctx.rel_hints(id);
    annot::Hints {
        prose_in: hints_rel.prose_in,
        prose_out: hints_rel
            .prose_out
            .map(|hint| alter::realign(&hint, input_hint)),
        ..annot::Hints::default()
    }
}

fn result_hints(ctx: &Context, input_hint: &input::InputHint) -> annot::Hints {
    let hints_rel = ctx.rel_hints(ctx.namespace());
    annot::Hints {
        prose_out: hints_rel
            .prose_out
            .map(|hint| alter::realign(&hint, input_hint)),
        ..annot::Hints::default()
    }
}

fn group_hints(ctx: &Context) -> annot::Hints {
    let hints_rel = ctx.rel_hints(ctx.namespace());
    annot::Hints {
        prose_in: hints_rel.prose_in,
        prose_true: hints_rel.prose_true,
        ..annot::Hints::default()
    }
}

fn func_def_hints(ctx: &Context, id: &sl::Id) -> annot::Hints {
    call_hints(ctx, id)
}

fn rel_def_hints(
    ctx: &Context,
    id: &sl::Id,
    rel_signature: &sl::RelSignature,
) -> Result<annot::Hints, ProseError> {
    let hints_rel = ctx.rel_hints(id);
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
            .map_err(|error| ProseError::new(ProseErrorKind::Input(error), id.span.clone()))?;
        let fresh_exps = |typs: Vec<sl::Typ>| {
            let mut ids_used = IdSet::new();
            typs.into_iter()
                .map(|typ| {
                    let (ids_fresh, exp_sl) =
                        al::fresh::exp_from_typ(true, ctx.metavars(), &ids_used, &typ);
                    ids_used = ids_fresh;
                    exp_sl
                })
                .collect::<Vec<_>>()
        };
        let exps_input = Some(fresh_exps(typs_input));
        let exps_output = prose_out.is_some().then(|| fresh_exps(typs_output));
        (exps_input, exps_output)
    } else {
        (None, None)
    };
    Ok(annot::Hints {
        prose: hints_rel.prose,
        prose_in: hints_rel.prose_in,
        prose_out,
        prose_true: hints_rel.prose_true,
        prose_false: hints_rel.prose_false,
        prose_input_exps,
        prose_output_exps,
        prose_fields: None,
    })
}

fn exp(ctx: &Context, exp_sl: &sl::Exp) -> Result<pl::Exp, ProseError> {
    let exp_kind_pl = match &exp_sl.node {
        il::ExpKind::Bool(value) => pl::ExpKind::Bool(*value),
        il::ExpKind::Num(num) => pl::ExpKind::Num(num.clone()),
        il::ExpKind::Text(text) => pl::ExpKind::Text(text.clone()),
        il::ExpKind::Var(id) => pl::ExpKind::Var(id.clone()),
        il::ExpKind::Un(op, op_typ, exp_inner_sl) => {
            pl::ExpKind::Un(*op, *op_typ, Box::new(exp(ctx, exp_inner_sl)?))
        }
        il::ExpKind::Bin(op, op_typ, exp_l_sl, exp_r_sl) => pl::ExpKind::Bin(
            *op,
            *op_typ,
            Box::new(exp(ctx, exp_l_sl)?),
            Box::new(exp(ctx, exp_r_sl)?),
        ),
        il::ExpKind::Cmp(op, op_typ, exp_l_sl, exp_r_sl) => pl::ExpKind::Cmp(
            *op,
            *op_typ,
            Box::new(exp(ctx, exp_l_sl)?),
            Box::new(exp(ctx, exp_r_sl)?),
        ),
        il::ExpKind::UpCast(typ, exp_inner_sl) => {
            pl::ExpKind::UpCast(typ.as_ref().clone(), Box::new(exp(ctx, exp_inner_sl)?))
        }
        il::ExpKind::DownCast(typ, exp_inner_sl) => {
            pl::ExpKind::DownCast(typ.as_ref().clone(), Box::new(exp(ctx, exp_inner_sl)?))
        }
        il::ExpKind::Sub(exp_inner_sl, typ, subcheck) => pl::ExpKind::Sub(
            Box::new(exp(ctx, exp_inner_sl)?),
            typ.as_ref().clone(),
            subcheck.clone(),
        ),
        il::ExpKind::Match(exp_inner_sl, pattern) => {
            pl::ExpKind::Match(Box::new(exp(ctx, exp_inner_sl)?), pattern.clone())
        }
        il::ExpKind::Tuple(exps_sl) => pl::ExpKind::Tuple(exps(ctx, exps_sl)?),
        il::ExpKind::Case(not_exp_sl) => pl::ExpKind::Case(Box::new(not_exp(ctx, not_exp_sl)?)),
        il::ExpKind::Str(fields_sl) => {
            let fields_pl = fields_sl
                .iter()
                .map(|(atom, exp_sl)| Ok((atom.clone(), exp(ctx, exp_sl)?)))
                .collect::<Result<_, ProseError>>()?;
            pl::ExpKind::Str(fields_pl)
        }
        il::ExpKind::Opt(exp_opt_sl) => pl::ExpKind::Opt(
            exp_opt_sl
                .as_deref()
                .map(|exp_sl| exp(ctx, exp_sl).map(Box::new))
                .transpose()?,
        ),
        il::ExpKind::List(exps_sl) => pl::ExpKind::List(exps(ctx, exps_sl)?),
        il::ExpKind::Cons(exp_l_sl, exp_r_sl) => {
            pl::ExpKind::Cons(Box::new(exp(ctx, exp_l_sl)?), Box::new(exp(ctx, exp_r_sl)?))
        }
        il::ExpKind::Cat(exp_l_sl, exp_r_sl) => {
            pl::ExpKind::Cat(Box::new(exp(ctx, exp_l_sl)?), Box::new(exp(ctx, exp_r_sl)?))
        }
        il::ExpKind::Mem(exp_l_sl, exp_r_sl) => {
            pl::ExpKind::Mem(Box::new(exp(ctx, exp_l_sl)?), Box::new(exp(ctx, exp_r_sl)?))
        }
        il::ExpKind::Len(exp_inner_sl) => pl::ExpKind::Len(Box::new(exp(ctx, exp_inner_sl)?)),
        il::ExpKind::Dot(exp_inner_sl, atom) => {
            pl::ExpKind::Dot(Box::new(exp(ctx, exp_inner_sl)?), atom.clone())
        }
        il::ExpKind::Idx(exp_l_sl, exp_r_sl) => {
            pl::ExpKind::Idx(Box::new(exp(ctx, exp_l_sl)?), Box::new(exp(ctx, exp_r_sl)?))
        }
        il::ExpKind::Slice(exp_base_sl, exp_idx_sl, exp_len_sl) => pl::ExpKind::Slice(
            Box::new(exp(ctx, exp_base_sl)?),
            Box::new(exp(ctx, exp_idx_sl)?),
            Box::new(exp(ctx, exp_len_sl)?),
        ),
        il::ExpKind::Upd(exp_base_sl, path_sl, exp_field_sl) => pl::ExpKind::Upd(
            Box::new(exp(ctx, exp_base_sl)?),
            Box::new(path(ctx, path_sl)?),
            Box::new(exp(ctx, exp_field_sl)?),
        ),
        il::ExpKind::Call(id, targs, args_sl) => pl::ExpKind::Call(
            id.clone(),
            targs.clone(),
            args_sl
                .iter()
                .map(|arg_sl| arg(ctx, arg_sl))
                .collect::<Result<_, _>>()?,
        ),
        il::ExpKind::Iter(exp_inner_sl, iter_exp) => {
            pl::ExpKind::Iter(Box::new(exp(ctx, exp_inner_sl)?), iter_exp.clone())
        }
    };
    let hints = match &exp_sl.node {
        il::ExpKind::Case(not_exp_sl) => {
            let hints = case_hints(ctx, exp_sl, not_exp_sl);
            let arity = not_exp_sl.args().len();
            validate_alterations(&exp_sl.span, &hints, arity)?;
            validate_fields(&exp_sl.span, &hints, arity)?;
            hints
        }
        il::ExpKind::Call(id, _, args_sl) => {
            let hints = call_hints(ctx, id);
            validate_alterations(&exp_sl.span, &hints, args_sl.len())?;
            hints
        }
        _ => annot::Hints::default(),
    };
    Ok(annot::Annotated {
        node: crate::note_phrase! {
            node: exp_kind_pl,
            note: exp_sl.note.as_ref().clone(),
            span: exp_sl.span.clone(),
        },
        hints,
    })
}

fn exps(ctx: &Context, exps_sl: &[sl::Exp]) -> Result<Vec<pl::Exp>, ProseError> {
    exps_sl.iter().map(|exp_sl| exp(ctx, exp_sl)).collect()
}

fn not_exp(ctx: &Context, not_exp_sl: &sl::NotExp) -> Result<pl::NotExp, ProseError> {
    Ok(match not_exp_sl {
        Mixfix::Arg(exp_sl) => Mixfix::Arg(exp(ctx, exp_sl)?),
        Mixfix::Atom(atom) => Mixfix::Atom(atom.clone()),
        Mixfix::Brack(atom_l, not_exp_inner_sl, atom_r) => {
            Mixfix::Brack(atom_l.clone(), Box::new(not_exp(ctx, not_exp_inner_sl)?), atom_r.clone())
        }
        Mixfix::Infix(not_exp_l_sl, atom, not_exp_r_sl) => Mixfix::Infix(
            Box::new(not_exp(ctx, not_exp_l_sl)?),
            atom.clone(),
            Box::new(not_exp(ctx, not_exp_r_sl)?),
        ),
        Mixfix::Seq(not_exps_sl) => Mixfix::Seq(
            not_exps_sl
                .iter()
                .map(|not_exp_sl| not_exp(ctx, not_exp_sl))
                .collect::<Result<_, _>>()?,
        ),
    })
}

fn path(ctx: &Context, path_sl: &sl::Path) -> Result<pl::Path, ProseError> {
    let path_kind_pl = match &path_sl.node {
        il::PathKind::Root => pl::PathKind::Root,
        il::PathKind::Idx(path_inner_sl, exp_idx_sl) => {
            pl::PathKind::Idx(Box::new(path(ctx, path_inner_sl)?), Box::new(exp(ctx, exp_idx_sl)?))
        }
        il::PathKind::Slice(path_inner_sl, exp_idx_sl, exp_len_sl) => pl::PathKind::Slice(
            Box::new(path(ctx, path_inner_sl)?),
            Box::new(exp(ctx, exp_idx_sl)?),
            Box::new(exp(ctx, exp_len_sl)?),
        ),
        il::PathKind::Dot(path_inner_sl, atom) => {
            pl::PathKind::Dot(Box::new(path(ctx, path_inner_sl)?), atom.clone())
        }
    };
    Ok(crate::note_phrase! {
        node: path_kind_pl,
        note: path_sl.note.as_ref().clone(),
        span: path_sl.span.clone(),
    })
}

fn arg(ctx: &Context, arg_sl: &sl::Arg) -> Result<pl::Arg, ProseError> {
    let arg_kind_pl = match &arg_sl.node {
        il::ArgKind::Exp(exp_sl) => pl::ArgKind::Exp(Box::new(exp(ctx, exp_sl)?)),
        il::ArgKind::Def(id) => pl::ArgKind::Def(id.clone()),
    };
    Ok(crate::phrase! { node: arg_kind_pl, span: arg_sl.span.clone() })
}

fn param(ctx: &Context, param_sl: &sl::Param) -> Result<pl::Param, ProseError> {
    let param_kind_pl = match &param_sl.node {
        sl::ParamKind::Exp(typ, exp_sl) => {
            pl::ParamKind::Exp(typ.clone(), Box::new(exp(ctx, exp_sl)?))
        }
        sl::ParamKind::Def(id, tparams, params_sl, typ) => pl::ParamKind::Def(
            id.clone(),
            tparams.clone(),
            params_sl
                .iter()
                .map(|param_sl| param(ctx, param_sl))
                .collect::<Result<_, _>>()?,
            typ.clone(),
        ),
    };
    Ok(crate::phrase! { node: param_kind_pl, span: param_sl.span.clone() })
}

fn guard(ctx: &Context, guard_sl: &sl::Guard) -> Result<pl::Guard, ProseError> {
    Ok(match guard_sl {
        sl::Guard::Bool(value) => pl::Guard::Bool(*value),
        sl::Guard::Cmp(op, op_typ, exp_sl) => pl::Guard::Cmp(*op, *op_typ, exp(ctx, exp_sl)?),
        sl::Guard::Sub(typ, subcheck) => pl::Guard::Sub(typ.clone(), subcheck.clone()),
        sl::Guard::Match(pattern) => pl::Guard::Match(pattern.clone()),
        sl::Guard::Mem(exp_sl) => pl::Guard::Mem(exp(ctx, exp_sl)?),
    })
}

fn instr<Tier>(instr_kind_pl: pl::InstrKind<Tier>, span: Span) -> pl::Instr<Tier> {
    instr_with_hints(instr_kind_pl, span, annot::Hints::default())
}

fn instr_with_hints<Tier>(
    instr_kind_pl: pl::InstrKind<Tier>,
    span: Span,
    hints: annot::Hints,
) -> pl::Instr<Tier> {
    annot::Annotated {
        node: crate::note_phrase! {
            node: instr_kind_pl,
            note: None,
            span: span,
        },
        hints,
    }
}

fn let_hints(exp_l_pl: &pl::Exp) -> annot::Hints {
    if matches!(exp_l_pl.node.node, pl::ExpKind::Case(_)) {
        annot::Hints {
            prose_fields: exp_l_pl.hints.prose_fields.clone(),
            ..annot::Hints::default()
        }
    } else {
        annot::Hints::default()
    }
}

fn block_dispatch(ctx: &Context, block_sl: sl::Block) -> Result<pl::BlockDispatch, ProseError> {
    match block_sl.len() {
        0 => Ok(Vec::new()),
        1 => instr_dispatch(ctx, block_sl.into_iter().next().unwrap()),
        _ => {
            let span = Span::over(
                &block_sl
                    .iter()
                    .map(|instr_sl| instr_sl.span.clone())
                    .collect::<Vec<_>>(),
            );
            let blocks = block_sl
                .into_iter()
                .map(|instr_sl| instr_dispatch(ctx, instr_sl))
                .collect::<Result<_, _>>()?;
            Ok(vec![instr(
                pl::InstrKind::Tier(pl::TierInstr {
                    tier: pl::InstrDispatch::Route(pl::RouteDispatchInstr { blocks }),
                }),
                span,
            )])
        }
    }
}

fn block_group(ctx: &Context, block_sl: sl::Block) -> Result<pl::BlockGroup, ProseError> {
    match block_sl.len() {
        0 => Ok(Vec::new()),
        1 => instr_group(ctx, block_sl.into_iter().next().unwrap()),
        _ => {
            let span = Span::over(
                &block_sl
                    .iter()
                    .map(|instr_sl| instr_sl.span.clone())
                    .collect::<Vec<_>>(),
            );
            let blocks = block_sl
                .into_iter()
                .map(|instr_sl| instr_group(ctx, instr_sl))
                .collect::<Result<_, _>>()?;
            Ok(vec![instr(
                pl::InstrKind::Tier(pl::TierInstr {
                    tier: pl::InstrGroup::Backtrack(pl::BacktrackGroupInstr { blocks }),
                }),
                span,
            )])
        }
    }
}

fn hold_case_dispatch(
    ctx: &Context,
    hold_case_sl: sl::HoldCase,
) -> Result<pl::HoldCase<pl::InstrDispatch>, ProseError> {
    Ok(match hold_case_sl {
        sl::HoldCase::Both(block_hold_sl, block_not_hold_sl) => pl::HoldCase::Both(
            block_dispatch(ctx, block_hold_sl)?,
            block_dispatch(ctx, block_not_hold_sl)?,
        ),
        sl::HoldCase::Hold(block_sl, dangle) => {
            pl::HoldCase::Hold(block_dispatch(ctx, block_sl)?, dangle)
        }
        sl::HoldCase::NotHold(block_sl, dangle) => {
            pl::HoldCase::NotHold(block_dispatch(ctx, block_sl)?, dangle)
        }
    })
}

fn hold_case_group(
    ctx: &Context,
    hold_case_sl: sl::HoldCase,
) -> Result<pl::HoldCase<pl::InstrGroup>, ProseError> {
    Ok(match hold_case_sl {
        sl::HoldCase::Both(block_hold_sl, block_not_hold_sl) => pl::HoldCase::Both(
            block_group(ctx, block_hold_sl)?,
            block_group(ctx, block_not_hold_sl)?,
        ),
        sl::HoldCase::Hold(block_sl, dangle) => {
            pl::HoldCase::Hold(block_group(ctx, block_sl)?, dangle)
        }
        sl::HoldCase::NotHold(block_sl, dangle) => {
            pl::HoldCase::NotHold(block_group(ctx, block_sl)?, dangle)
        }
    })
}

fn instr_dispatch(ctx: &Context, instr_sl: sl::Instr) -> Result<pl::BlockDispatch, ProseError> {
    let span = instr_sl.span;
    Ok(match instr_sl.node {
        sl::InstrKind::If(instr_sl) => vec![instr(
            pl::InstrKind::If(pl::IfInstr {
                exp: exp(ctx, &instr_sl.exp)?,
                iter_exps: instr_sl.iter_exps,
                block: block_dispatch(ctx, instr_sl.block)?,
                dangle: instr_sl.dangle,
            }),
            span,
        )],
        sl::InstrKind::Hold(instr_sl) => {
            let hints = hold_hints(ctx, &instr_sl.id);
            validate_alterations(&span, &hints, instr_sl.not_exp.args().len())?;
            vec![instr_with_hints(
                pl::InstrKind::Hold(pl::HoldInstr {
                    id: instr_sl.id,
                    not_exp: not_exp(ctx, &instr_sl.not_exp)?,
                    iter_exps: instr_sl.iter_exps,
                    hold_case: hold_case_dispatch(ctx, instr_sl.hold_case)?,
                }),
                span,
                hints,
            )]
        }
        sl::InstrKind::Case(instr_sl) => vec![instr(
            pl::InstrKind::Case(pl::CaseInstr {
                exp: exp(ctx, &instr_sl.exp)?,
                cases: instr_sl
                    .cases
                    .into_iter()
                    .map(|case_sl| {
                        Ok(pl::Case {
                            guard: guard(ctx, &case_sl.guard)?,
                            block: block_dispatch(ctx, case_sl.block)?,
                        })
                    })
                    .collect::<Result<_, ProseError>>()?,
                dangle: instr_sl.dangle,
            }),
            span,
        )],
        sl::InstrKind::Let(instr_sl) => {
            let exp_l_pl = exp(ctx, &instr_sl.exp_l)?;
            let hints = let_hints(&exp_l_pl);
            let mut instrs_output = vec![instr_with_hints(
                pl::InstrKind::Let(pl::LetInstr {
                    exp_l: exp_l_pl,
                    exp_r: exp(ctx, &instr_sl.exp_r)?,
                    iter_instrs: instr_sl.iter_instrs,
                }),
                span,
                hints,
            )];
            instrs_output.extend(block_dispatch(ctx, instr_sl.block)?);
            instrs_output
        }
        sl::InstrKind::Debug(instr_sl) => {
            let mut instrs_output = vec![instr(
                pl::InstrKind::Debug(pl::DebugInstr { exp: exp(ctx, &instr_sl.exp)? }),
                span,
            )];
            instrs_output.extend(instr_dispatch(ctx, *instr_sl.instr)?);
            instrs_output
        }
        sl::InstrKind::Group(instr_sl) => {
            let hints = group_hints(ctx);
            input::validate(&instr_sl.rel_signature.input_hint, instr_sl.exps.len())
                .map_err(|error| ProseError::new(ProseErrorKind::Input(error), span.clone()))?;
            validate_alterations(&span, &hints, instr_sl.rel_signature.input_hint.indices().len())?;
            vec![instr_with_hints(
                pl::InstrKind::Tier(pl::TierInstr {
                    tier: pl::InstrDispatch::Group(pl::GroupDispatchInstr {
                        id_rel: ctx.namespace().clone(),
                        id_group: instr_sl.id,
                        rel_signature: instr_sl.rel_signature,
                        exps_input: exps(ctx, &instr_sl.exps)?,
                        block: block_group(ctx, instr_sl.block)?,
                    }),
                }),
                span,
                hints,
            )]
        }
        sl::InstrKind::Rule(_) | sl::InstrKind::Result(_) | sl::InstrKind::Return(_) => {
            return Err(ProseError::new(ProseErrorKind::InvalidDispatchTier, span));
        }
    })
}

fn instr_group(ctx: &Context, instr_sl: sl::Instr) -> Result<pl::BlockGroup, ProseError> {
    let span = instr_sl.span;
    Ok(match instr_sl.node {
        sl::InstrKind::If(instr_sl) => vec![instr(
            pl::InstrKind::If(pl::IfInstr {
                exp: exp(ctx, &instr_sl.exp)?,
                iter_exps: instr_sl.iter_exps,
                block: block_group(ctx, instr_sl.block)?,
                dangle: instr_sl.dangle,
            }),
            span,
        )],
        sl::InstrKind::Hold(instr_sl) => {
            let hints = hold_hints(ctx, &instr_sl.id);
            validate_alterations(&span, &hints, instr_sl.not_exp.args().len())?;
            vec![instr_with_hints(
                pl::InstrKind::Hold(pl::HoldInstr {
                    id: instr_sl.id,
                    not_exp: not_exp(ctx, &instr_sl.not_exp)?,
                    iter_exps: instr_sl.iter_exps,
                    hold_case: hold_case_group(ctx, instr_sl.hold_case)?,
                }),
                span,
                hints,
            )]
        }
        sl::InstrKind::Case(instr_sl) => vec![instr(
            pl::InstrKind::Case(pl::CaseInstr {
                exp: exp(ctx, &instr_sl.exp)?,
                cases: instr_sl
                    .cases
                    .into_iter()
                    .map(|case_sl| {
                        Ok(pl::Case {
                            guard: guard(ctx, &case_sl.guard)?,
                            block: block_group(ctx, case_sl.block)?,
                        })
                    })
                    .collect::<Result<_, ProseError>>()?,
                dangle: instr_sl.dangle,
            }),
            span,
        )],
        sl::InstrKind::Let(instr_sl) => {
            let exp_l_pl = exp(ctx, &instr_sl.exp_l)?;
            let hints = let_hints(&exp_l_pl);
            let mut instrs_output = vec![instr_with_hints(
                pl::InstrKind::Let(pl::LetInstr {
                    exp_l: exp_l_pl,
                    exp_r: exp(ctx, &instr_sl.exp_r)?,
                    iter_instrs: instr_sl.iter_instrs,
                }),
                span,
                hints,
            )];
            instrs_output.extend(block_group(ctx, instr_sl.block)?);
            instrs_output
        }
        sl::InstrKind::Debug(instr_sl) => {
            let mut instrs_output = vec![instr(
                pl::InstrKind::Debug(pl::DebugInstr { exp: exp(ctx, &instr_sl.exp)? }),
                span,
            )];
            instrs_output.extend(instr_group(ctx, *instr_sl.instr)?);
            instrs_output
        }
        sl::InstrKind::Rule(instr_sl) => {
            let hints = rule_hints(ctx, &instr_sl.id, &instr_sl.input_hint);
            let arity = instr_sl.not_exp.args().len();
            input::validate(&instr_sl.input_hint, arity)
                .map_err(|error| ProseError::new(ProseErrorKind::Input(error), span.clone()))?;
            let input_count = instr_sl.input_hint.indices().len();
            validate_split(&span, &hints, input_count, arity - input_count)?;
            let mut instrs_output = vec![instr_with_hints(
                pl::InstrKind::Tier(pl::TierInstr {
                    tier: pl::InstrGroup::Rule(pl::RuleGroupInstr {
                        id: instr_sl.id,
                        not_exp: not_exp(ctx, &instr_sl.not_exp)?,
                        input_hint: instr_sl.input_hint,
                        iter_instrs: instr_sl.iter_instrs,
                    }),
                }),
                span,
                hints,
            )];
            instrs_output.extend(block_group(ctx, instr_sl.block)?);
            instrs_output
        }
        sl::InstrKind::Result(instr_sl) => {
            let hints = result_hints(ctx, &instr_sl.rel_signature.input_hint);
            validate_alterations(&span, &hints, instr_sl.exps.len())?;
            vec![instr_with_hints(
                pl::InstrKind::Tier(pl::TierInstr {
                    tier: pl::InstrGroup::Result(pl::ResultGroupInstr {
                        rel_signature: instr_sl.rel_signature,
                        exps_output: exps(ctx, &instr_sl.exps)?,
                    }),
                }),
                span,
                hints,
            )]
        }
        sl::InstrKind::Return(instr_sl) => vec![instr(
            pl::InstrKind::Tier(pl::TierInstr {
                tier: pl::InstrGroup::Return(pl::ReturnGroupInstr {
                    exp: exp(ctx, &instr_sl.exp)?,
                }),
            }),
            span,
        )],
        sl::InstrKind::Group(_) => {
            return Err(ProseError::new(ProseErrorKind::InvalidGroupTier, span));
        }
    })
}

fn def(ctx: &mut Context, def_sl: sl::Def) -> Result<pl::Def, ProseError> {
    let hints = match &def_sl.node {
        sl::DefKind::Rel(sl::RelDef::Extern(def_rel_sl)) => {
            rel_def_hints(ctx, &def_rel_sl.id, &def_rel_sl.rel_signature)?
        }
        sl::DefKind::Rel(sl::RelDef::Defined(def_rel_sl)) => {
            rel_def_hints(ctx, &def_rel_sl.id, &def_rel_sl.rel_signature)?
        }
        sl::DefKind::MetaFunc(sl::MetaFuncDef::Extern(def_func_sl)) => {
            func_def_hints(ctx, &def_func_sl.id)
        }
        sl::DefKind::MetaFunc(sl::MetaFuncDef::Builtin(def_func_sl)) => {
            func_def_hints(ctx, &def_func_sl.id)
        }
        sl::DefKind::MetaFunc(sl::MetaFuncDef::Table(def_func_sl)) => {
            func_def_hints(ctx, &def_func_sl.id)
        }
        sl::DefKind::MetaFunc(sl::MetaFuncDef::Defined(def_func_sl)) => {
            func_def_hints(ctx, &def_func_sl.id)
        }
        sl::DefKind::Typ(_) | sl::DefKind::Var(_) => annot::Hints::default(),
    };
    let span = def_sl.span;
    let def_kind_pl = match def_sl.node {
        sl::DefKind::Typ(typ_def_sl) => pl::DefKind::Typ(match typ_def_sl {
            sl::TypDef::Extern(def_typ_sl) => {
                pl::TypDef::Extern(pl::ExternTyp { id: def_typ_sl.id })
            }
            sl::TypDef::Defined(def_typ_sl) => pl::TypDef::Defined(Box::new(pl::DefinedTyp {
                id: def_typ_sl.id,
                tparams: def_typ_sl.tparams,
                def_typ: def_typ_sl.def_typ,
            })),
        }),
        sl::DefKind::Var(def_var_sl) => {
            pl::DefKind::Var(pl::VarDef { id: def_var_sl.id, typ: def_var_sl.typ })
        }
        sl::DefKind::Rel(rel_def_sl) => pl::DefKind::Rel(match rel_def_sl {
            sl::RelDef::Extern(def_rel_sl) => pl::RelDef::Extern(pl::ExternRel {
                id: def_rel_sl.id,
                rel_signature: def_rel_sl.rel_signature,
                exps_input: exps(ctx, &def_rel_sl.exps_input)?,
            }),
            sl::RelDef::Defined(def_rel_sl) => {
                ctx.set_namespace(def_rel_sl.id.clone());
                pl::RelDef::Defined(pl::DefinedRel {
                    id: def_rel_sl.id,
                    rel_signature: def_rel_sl.rel_signature,
                    exps_input: exps(ctx, &def_rel_sl.exps_input)?,
                    block: block_dispatch(ctx, def_rel_sl.block)?,
                    block_else_opt: def_rel_sl
                        .block_else
                        .map(|block_sl| block_dispatch(ctx, block_sl))
                        .transpose()?,
                })
            }
        }),
        sl::DefKind::MetaFunc(meta_func_def_sl) => pl::DefKind::MetaFunc(match meta_func_def_sl {
            sl::MetaFuncDef::Extern(def_func_sl) => {
                ctx.validate_tparams(&def_func_sl.tparams)?;
                pl::MetaFuncDef::Extern(pl::ExternFunc {
                    id: def_func_sl.id,
                    tparams: def_func_sl.tparams,
                    params: def_func_sl
                        .params
                        .iter()
                        .map(|param_sl| param(ctx, param_sl))
                        .collect::<Result<_, _>>()?,
                    typ: def_func_sl.typ,
                })
            }
            sl::MetaFuncDef::Builtin(def_func_sl) => {
                ctx.validate_tparams(&def_func_sl.tparams)?;
                pl::MetaFuncDef::Builtin(pl::BuiltinFunc {
                    id: def_func_sl.id,
                    tparams: def_func_sl.tparams,
                    params: def_func_sl
                        .params
                        .iter()
                        .map(|param_sl| param(ctx, param_sl))
                        .collect::<Result<_, _>>()?,
                    typ: def_func_sl.typ,
                })
            }
            sl::MetaFuncDef::Table(def_func_sl) => {
                ctx.set_namespace(def_func_sl.id.clone());
                pl::MetaFuncDef::Table(pl::TableFunc {
                    id: def_func_sl.id,
                    params: def_func_sl
                        .params
                        .iter()
                        .map(|param_sl| param(ctx, param_sl))
                        .collect::<Result<_, _>>()?,
                    typ: def_func_sl.typ,
                    rows: def_func_sl
                        .table_rows
                        .into_iter()
                        .map(|row_sl| {
                            Ok(pl::TableRow {
                                exps_input: exps(ctx, &row_sl.exps_input)?,
                                exp: exp(ctx, &row_sl.exp)?,
                                block: block_group(ctx, row_sl.block)?,
                            })
                        })
                        .collect::<Result<_, ProseError>>()?,
                })
            }
            sl::MetaFuncDef::Defined(def_func_sl) => {
                ctx.validate_tparams(&def_func_sl.tparams)?;
                ctx.set_namespace(def_func_sl.id.clone());
                pl::MetaFuncDef::Defined(pl::DefinedFunc {
                    id: def_func_sl.id,
                    tparams: def_func_sl.tparams,
                    params: def_func_sl
                        .params
                        .iter()
                        .map(|param_sl| param(ctx, param_sl))
                        .collect::<Result<_, _>>()?,
                    typ: def_func_sl.typ,
                    block: block_group(ctx, def_func_sl.block)?,
                    block_else_opt: def_func_sl
                        .block_else
                        .map(|block_sl| block_group(ctx, block_sl))
                        .transpose()?,
                })
            }
        }),
    };
    Ok(annot::Annotated { node: crate::phrase! { node: def_kind_pl, span: span }, hints })
}

pub(super) fn convert(spec_sl: sl::Spec) -> Result<pl::Spec, ProseError> {
    let mut ctx = Context::load(&spec_sl)?;
    let mut spec_pl = Vec::with_capacity(spec_sl.len());
    for def_sl in super::expand::spec(spec_sl)? {
        spec_pl.push(def(&mut ctx, def_sl)?);
    }
    Ok(super::stamp::spec(super::shorthand::spec(spec_pl)))
}
