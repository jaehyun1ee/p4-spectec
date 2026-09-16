//! Give used underscore-prefixed inputs and OL binders ordinary names
//!
//! `candid_renamer` strips leading underscores and chooses fresh names;
//! `downstream_block` records which candidates are actually used
//! When `x` is available:
//!
//! ```text
//! let _x = source { return _x }
//!
//! becomes
//!
//! let x = source { return x }
//! ```
//!
//! An unused `_x` keeps its name; a conflicting `x` forces a fresh name
//! `apply_rel` and `apply_func` also check uses of inputs in the fallback,
//! while nested bindings of the same underscore name stop substitution

use super::super::{StructureError, StructureErrorKind, ol::ast::*, re::renamer::Renamer};
use crate::lang::{
    common::{ds::set::IdSet, source::Span},
    hints::input,
    il::{
        ast::{Arg, Exp},
        fresh,
    },
    traits::free::Free,
};

// == Candidate names

fn underscores(frees: IdSet) -> IdSet {
    frees
        .iter()
        .filter(|id| id.node.starts_with('_'))
        .cloned()
        .collect()
}

fn candid_renamer(mut frees: IdSet, ids: &IdSet) -> (IdSet, Renamer) {
    let mut renamer = Renamer::empty();
    for id in ids.iter() {
        let mut id_strip = id.clone();
        id_strip.node = id_strip.node.trim_start_matches('_').to_owned();
        let id_revive = fresh::id(&frees, &id_strip);
        frees.insert(id_revive.clone());
        renamer.add(id.clone(), id_revive);
    }
    (frees, renamer)
}

fn mark_used(renamer: &Renamer, ids_revive: &mut IdSet, frees: IdSet) {
    ids_revive.append(renamer.dom().intersection(&underscores(frees)));
}

// == Downstream uses

fn downstream_exp(renamer: &Renamer, ids_revive: &mut IdSet, exp: Exp) -> Exp {
    mark_used(renamer, ids_revive, exp.free());
    renamer.rename_exp(exp)
}

fn downstream_exps(renamer: &Renamer, ids_revive: &mut IdSet, exps: Vec<Exp>) -> Vec<Exp> {
    mark_used(renamer, ids_revive, exps.as_slice().free());
    renamer.rename_exps(exps)
}

fn downstream_guard(renamer: &Renamer, ids_revive: &mut IdSet, guard: Guard) -> Guard {
    mark_used(renamer, ids_revive, guard.free());
    renamer.rename_guard(guard)
}

fn downstream_instr(
    renamer: &Renamer,
    ids_revive: &mut IdSet,
    instr_ol: Instr,
) -> Result<Instr, StructureError> {
    let instr_kind_ol = downstream_instr_kind(renamer, ids_revive, instr_ol.node, &instr_ol.span)?;
    Ok(crate::phrase!(node: instr_kind_ol, span: instr_ol.span))
}

fn downstream_instr_kind(
    renamer: &Renamer,
    ids_revive: &mut IdSet,
    instr_kind_ol: InstrKind,
    span: &Span,
) -> Result<InstrKind, StructureError> {
    match instr_kind_ol {
        InstrKind::If(instr_ol) => downstream_if_instr(renamer, ids_revive, instr_ol),
        InstrKind::Hold(instr_ol) => downstream_hold_instr(renamer, ids_revive, instr_ol),
        InstrKind::Case(instr_ol) => downstream_case_instr(renamer, ids_revive, instr_ol),
        InstrKind::Group(instr_ol) => downstream_group_instr(renamer, ids_revive, instr_ol),
        InstrKind::Let(instr_ol) => downstream_let_instr(renamer, ids_revive, instr_ol),
        InstrKind::Rule(instr_ol) => downstream_rule_instr(renamer, ids_revive, instr_ol, span),
        InstrKind::Result(instr_ol) => downstream_result_instr(renamer, ids_revive, instr_ol),
        InstrKind::Return(instr_ol) => downstream_return_instr(renamer, ids_revive, instr_ol),
        InstrKind::Debug(instr_ol) => downstream_debug_instr(renamer, ids_revive, instr_ol),
    }
}

fn downstream_if_instr(
    renamer: &Renamer,
    ids_revive: &mut IdSet,
    instr_ol: IfInstr,
) -> Result<InstrKind, StructureError> {
    let IfInstr {
        exp,
        iter_exps,
        block,
    } = instr_ol;
    let exp = downstream_exp(renamer, ids_revive, exp);
    let iter_exps = renamer.rename_iterexps(iter_exps);
    let block = downstream_block(renamer, ids_revive, block)?;
    Ok(InstrKind::If(IfInstr {
        exp,
        iter_exps,
        block,
    }))
}

fn downstream_hold_instr(
    renamer: &Renamer,
    ids_revive: &mut IdSet,
    instr_ol: HoldInstr,
) -> Result<InstrKind, StructureError> {
    let HoldInstr {
        id,
        not_exp,
        iter_exps,
        block_hold,
        block_not_hold,
    } = instr_ol;
    let not_exp = not_exp.map(|exp| downstream_exp(renamer, ids_revive, exp.clone()));
    let iter_exps = renamer.rename_iterexps(iter_exps);
    let block_hold = downstream_block(renamer, ids_revive, block_hold)?;
    let block_not_hold = downstream_block(renamer, ids_revive, block_not_hold)?;
    Ok(InstrKind::Hold(HoldInstr {
        id,
        not_exp,
        iter_exps,
        block_hold,
        block_not_hold,
    }))
}

fn downstream_case_instr(
    renamer: &Renamer,
    ids_revive: &mut IdSet,
    instr_ol: CaseInstr,
) -> Result<InstrKind, StructureError> {
    let CaseInstr { exp, cases, total } = instr_ol;
    let exp = downstream_exp(renamer, ids_revive, exp);
    let cases = cases
        .into_iter()
        .map(|case| downstream_case(renamer, ids_revive, case))
        .collect::<Result<_, _>>()?;
    Ok(InstrKind::Case(CaseInstr { exp, cases, total }))
}

fn downstream_case(
    renamer: &Renamer,
    ids_revive: &mut IdSet,
    case: Case,
) -> Result<Case, StructureError> {
    let Case { guard, block } = case;
    let guard = downstream_guard(renamer, ids_revive, guard);
    let block = downstream_block(renamer, ids_revive, block)?;
    Ok(Case { guard, block })
}

fn downstream_group_instr(
    renamer: &Renamer,
    ids_revive: &mut IdSet,
    instr_ol: GroupInstr,
) -> Result<InstrKind, StructureError> {
    let GroupInstr {
        id,
        rel_signature,
        exps,
        block,
    } = instr_ol;
    let exps = downstream_exps(renamer, ids_revive, exps);
    let block = downstream_block(renamer, ids_revive, block)?;
    Ok(InstrKind::Group(GroupInstr {
        id,
        rel_signature,
        exps,
        block,
    }))
}

fn downstream_let_instr(
    renamer: &Renamer,
    ids_revive: &mut IdSet,
    instr_ol: LetInstr,
) -> Result<InstrKind, StructureError> {
    let LetInstr {
        exp_l,
        exp_r,
        iter_instrs,
        block,
    } = instr_ol;
    let exp_r = downstream_exp(renamer, ids_revive, exp_r);
    let iter_instrs = renamer.rename_iterinstrs_bound(iter_instrs);
    let ids_bound = underscores(exp_l.free());
    let renamer = renamer.filter(|id, _| !ids_bound.contains(id));
    let block = downstream_block(&renamer, ids_revive, block)?;
    Ok(InstrKind::Let(LetInstr {
        exp_l,
        exp_r,
        iter_instrs,
        block,
    }))
}

fn downstream_rule_instr(
    renamer: &Renamer,
    ids_revive: &mut IdSet,
    instr_ol: RuleInstr,
    span: &Span,
) -> Result<InstrKind, StructureError> {
    let RuleInstr {
        id,
        not_exp,
        input_hint,
        iter_instrs,
        block,
    } = instr_ol;
    let (exps_input, exps_output) = split_rule(&not_exp, &input_hint, span)?;
    let exps_input = downstream_exps(renamer, ids_revive, exps_input);
    let ids_bound = underscores(exps_output.as_slice().free());
    let not_exp = fill_rule(not_exp, &input_hint, exps_input, exps_output, span)?;
    let iter_instrs = renamer.rename_iterinstrs_bound(iter_instrs);
    let renamer = renamer.filter(|id, _| !ids_bound.contains(id));
    let block = downstream_block(&renamer, ids_revive, block)?;
    Ok(InstrKind::Rule(RuleInstr {
        id,
        not_exp,
        input_hint,
        iter_instrs,
        block,
    }))
}

fn downstream_result_instr(
    renamer: &Renamer,
    ids_revive: &mut IdSet,
    instr_ol: ResultInstr,
) -> Result<InstrKind, StructureError> {
    let ResultInstr {
        rel_signature,
        exps,
    } = instr_ol;
    let exps = downstream_exps(renamer, ids_revive, exps);
    Ok(InstrKind::Result(ResultInstr {
        rel_signature,
        exps,
    }))
}

fn downstream_return_instr(
    renamer: &Renamer,
    ids_revive: &mut IdSet,
    instr_ol: ReturnInstr,
) -> Result<InstrKind, StructureError> {
    let ReturnInstr { exp } = instr_ol;
    let exp = downstream_exp(renamer, ids_revive, exp);
    Ok(InstrKind::Return(ReturnInstr { exp }))
}

fn downstream_debug_instr(
    renamer: &Renamer,
    ids_revive: &mut IdSet,
    instr_ol: DebugInstr,
) -> Result<InstrKind, StructureError> {
    let DebugInstr { exp, instr } = instr_ol;
    let exp = downstream_exp(renamer, ids_revive, exp);
    let instr = Box::new(downstream_instr(renamer, ids_revive, *instr)?);
    Ok(InstrKind::Debug(DebugInstr { exp, instr }))
}

fn downstream_block(
    renamer: &Renamer,
    ids_revive: &mut IdSet,
    block: Block,
) -> Result<Block, StructureError> {
    block
        .into_iter()
        .map(|instr_ol| downstream_instr(renamer, ids_revive, instr_ol))
        .collect()
}

// == Upstream bindings

fn upstream_instr(frees: &IdSet, instr_ol: Instr) -> Result<Instr, StructureError> {
    let instr_kind_ol = upstream_instr_kind(frees, instr_ol.node, &instr_ol.span)?;
    Ok(crate::phrase!(node: instr_kind_ol, span: instr_ol.span))
}

fn upstream_instr_kind(
    frees: &IdSet,
    instr_kind_ol: InstrKind,
    span: &Span,
) -> Result<InstrKind, StructureError> {
    match instr_kind_ol {
        InstrKind::If(instr_ol) => upstream_if_instr(frees, instr_ol),
        InstrKind::Hold(instr_ol) => upstream_hold_instr(frees, instr_ol),
        InstrKind::Case(instr_ol) => upstream_case_instr(frees, instr_ol),
        InstrKind::Group(instr_ol) => upstream_group_instr(frees, instr_ol),
        InstrKind::Let(instr_ol) => upstream_let_instr(frees, instr_ol),
        InstrKind::Rule(instr_ol) => upstream_rule_instr(frees, instr_ol, span),
        _ => Ok(instr_kind_ol),
    }
}

fn upstream_if_instr(frees: &IdSet, instr_ol: IfInstr) -> Result<InstrKind, StructureError> {
    let IfInstr {
        exp,
        iter_exps,
        block,
    } = instr_ol;
    let block = upstream_block(frees, block)?;
    Ok(InstrKind::If(IfInstr {
        exp,
        iter_exps,
        block,
    }))
}

fn upstream_hold_instr(frees: &IdSet, instr_ol: HoldInstr) -> Result<InstrKind, StructureError> {
    let HoldInstr {
        id,
        not_exp,
        iter_exps,
        block_hold,
        block_not_hold,
    } = instr_ol;
    let block_hold = upstream_block(frees, block_hold)?;
    let block_not_hold = upstream_block(frees, block_not_hold)?;
    Ok(InstrKind::Hold(HoldInstr {
        id,
        not_exp,
        iter_exps,
        block_hold,
        block_not_hold,
    }))
}

fn upstream_case_instr(frees: &IdSet, instr_ol: CaseInstr) -> Result<InstrKind, StructureError> {
    let CaseInstr { exp, cases, total } = instr_ol;
    let cases = cases
        .into_iter()
        .map(|case| upstream_case(frees, case))
        .collect::<Result<_, _>>()?;
    Ok(InstrKind::Case(CaseInstr { exp, cases, total }))
}

fn upstream_case(frees: &IdSet, case: Case) -> Result<Case, StructureError> {
    let Case { guard, block } = case;
    let block = upstream_block(frees, block)?;
    Ok(Case { guard, block })
}

fn upstream_group_instr(frees: &IdSet, instr_ol: GroupInstr) -> Result<InstrKind, StructureError> {
    let GroupInstr {
        id,
        rel_signature,
        exps,
        block,
    } = instr_ol;
    let block = upstream_block(frees, block)?;
    Ok(InstrKind::Group(GroupInstr {
        id,
        rel_signature,
        exps,
        block,
    }))
}

fn upstream_let_instr(frees: &IdSet, instr_ol: LetInstr) -> Result<InstrKind, StructureError> {
    let LetInstr {
        exp_l,
        exp_r,
        iter_instrs,
        block,
    } = instr_ol;
    let (_, renamer) = candid_renamer(frees.clone(), &underscores(exp_l.free()));
    let mut ids_revive = IdSet::new();
    let block = downstream_block(&renamer, &mut ids_revive, block)?;
    let renamer = renamer.filter(|id, _| ids_revive.contains(id));
    let exp_l = renamer.rename_exp(exp_l);
    let iter_instrs = renamer.rename_iterinstrs_bind(iter_instrs);
    let block = renamer.rename_block(block)?;
    Ok(InstrKind::Let(LetInstr {
        exp_l,
        exp_r,
        iter_instrs,
        block,
    }))
}

fn upstream_rule_instr(
    frees: &IdSet,
    instr_ol: RuleInstr,
    span: &Span,
) -> Result<InstrKind, StructureError> {
    let RuleInstr {
        id,
        not_exp,
        input_hint,
        iter_instrs,
        block,
    } = instr_ol;
    let (exps_input, exps_output) = split_rule(&not_exp, &input_hint, span)?;
    let (_, renamer) = candid_renamer(frees.clone(), &underscores(exps_output.as_slice().free()));
    let mut ids_revive = IdSet::new();
    let block = downstream_block(&renamer, &mut ids_revive, block)?;
    let renamer = renamer.filter(|id, _| ids_revive.contains(id));
    let exps_output = renamer.rename_exps(exps_output);
    let not_exp = fill_rule(not_exp, &input_hint, exps_input, exps_output, span)?;
    let iter_instrs = renamer.rename_iterinstrs_bind(iter_instrs);
    Ok(InstrKind::Rule(RuleInstr {
        id,
        not_exp,
        input_hint,
        iter_instrs,
        block,
    }))
}

fn upstream_block(frees: &IdSet, block: Block) -> Result<Block, StructureError> {
    block
        .into_iter()
        .map(|instr_ol| upstream_instr(frees, instr_ol))
        .collect()
}

fn split_rule(
    not_exp: &NotExp,
    input_hint: &input::InputHint,
    span: &Span,
) -> Result<(Vec<Exp>, Vec<Exp>), StructureError> {
    let exps = not_exp.args().into_iter().cloned().collect();
    input::split(input_hint, exps)
        .map_err(|error| StructureError::new(StructureErrorKind::Input(error), span.clone()))
}

fn fill_rule(
    not_exp: NotExp,
    input_hint: &input::InputHint,
    exps_input: Vec<Exp>,
    exps_output: Vec<Exp>,
    span: &Span,
) -> Result<NotExp, StructureError> {
    let exps = input::combine(input_hint, exps_input, exps_output)
        .map_err(|error| StructureError::new(StructureErrorKind::Input(error), span.clone()))?;
    let mut idx = 0;
    Ok(not_exp.map(|_| {
        let exp = exps[idx].clone();
        idx += 1;
        exp
    }))
}

// == Entry points

pub(crate) fn apply_rel(
    (mut exps_match, mut block, mut block_else): (Vec<Exp>, Block, Option<Block>),
) -> Result<(Vec<Exp>, Block, Option<Block>), StructureError> {
    let frees_input = exps_match.as_slice().free();
    let ids_bound = underscores(frees_input.clone());
    let frees = frees_input
        .union(block.free())
        .union(block_else.as_ref().map(Free::free).unwrap_or_default());
    let (frees, renamer) = candid_renamer(frees, &ids_bound);
    let mut ids_revive = IdSet::new();
    block = downstream_block(&renamer, &mut ids_revive, block)?;
    block_else = block_else
        .map(|block| downstream_block(&renamer, &mut ids_revive, block))
        .transpose()?;
    let renamer = renamer.filter(|id, _| ids_revive.contains(id));
    exps_match = renamer.rename_exps(exps_match);
    block = upstream_block(&frees, block)?;
    block_else = block_else
        .map(|block| upstream_block(&frees, block))
        .transpose()?;
    Ok((exps_match, block, block_else))
}

pub(crate) fn apply_func(
    (mut args_input, mut block, mut block_else): (Vec<Arg>, Block, Option<Block>),
) -> Result<(Vec<Arg>, Block, Option<Block>), StructureError> {
    let frees_input = args_input.as_slice().free();
    let ids_bound = underscores(frees_input.clone());
    let frees = frees_input
        .union(block.free())
        .union(block_else.as_ref().map(Free::free).unwrap_or_default());
    let (frees, renamer) = candid_renamer(frees, &ids_bound);
    let mut ids_revive = IdSet::new();
    block = downstream_block(&renamer, &mut ids_revive, block)?;
    block_else = block_else
        .map(|block| downstream_block(&renamer, &mut ids_revive, block))
        .transpose()?;
    let renamer = renamer.filter(|id, _| ids_revive.contains(id));
    args_input = renamer.rename_args(args_input);
    block = upstream_block(&frees, block)?;
    block_else = block_else
        .map(|block| upstream_block(&frees, block))
        .transpose()?;
    Ok((args_input, block, block_else))
}
