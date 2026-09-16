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

use crate::lang::{
    common::{ds::set::IdSet, source::Span},
    hints::input,
    il::{
        ast::{Arg, Exp, Mixop},
        fresh,
    },
    traits::free::Free,
};
use crate::pass::structure::{
    StructureError, StructureErrorKind, ol::ast::*, re::renamer::Renamer,
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

// == Downstream uses

// With {_x -> x}, `return _x` becomes `return x` and adds _x to ids_revive
// A nested `let _x = value { return _x }` keeps its own _x inside the body

// - Expressions

fn downstream_exp(renamer: &Renamer, ids_revive: &mut IdSet, exp: Exp) -> Exp {
    let ids_used = underscores(exp.free());
    ids_revive.append(renamer.dom().intersection(&ids_used));
    renamer.rename_exp(exp)
}

fn downstream_exps(renamer: &Renamer, ids_revive: &mut IdSet, exps: Vec<Exp>) -> Vec<Exp> {
    let ids_used = underscores(exps.as_slice().free());
    ids_revive.append(renamer.dom().intersection(&ids_used));
    renamer.rename_exps(exps)
}

// - Instructions

fn downstream_instr(
    renamer: &Renamer,
    ids_revive: &mut IdSet,
    instr_ol: Instr,
) -> Result<Instr, StructureError> {
    let instr_kind_ol = downstream_instr_kind(renamer, ids_revive, instr_ol.node, &instr_ol.span)?;
    let instr = crate::phrase!(node: instr_kind_ol, span: instr_ol.span);
    Ok(instr)
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

// - If instruction

fn downstream_if_instr(
    renamer: &Renamer,
    ids_revive: &mut IdSet,
    instr_ol: IfInstr,
) -> Result<InstrKind, StructureError> {
    let IfInstr { exp, iter_exps, block } = instr_ol;
    let exp = downstream_exp(renamer, ids_revive, exp);
    let iter_exps = renamer.rename_iterexps(iter_exps);
    let block = downstream_block(renamer, ids_revive, block)?;
    let instr = IfInstr { exp, iter_exps, block };
    Ok(InstrKind::If(instr))
}

// - Hold instruction

fn downstream_hold_instr(
    renamer: &Renamer,
    ids_revive: &mut IdSet,
    instr_ol: HoldInstr,
) -> Result<InstrKind, StructureError> {
    let HoldInstr { id, not_exp, iter_exps, block_hold, block_not_hold } = instr_ol;
    let not_exp = not_exp.map(|exp| downstream_exp(renamer, ids_revive, exp.clone()));
    let iter_exps = renamer.rename_iterexps(iter_exps);
    let block_hold = downstream_block(renamer, ids_revive, block_hold)?;
    let block_not_hold = downstream_block(renamer, ids_revive, block_not_hold)?;
    let instr = HoldInstr { id, not_exp, iter_exps, block_hold, block_not_hold };
    Ok(InstrKind::Hold(instr))
}

// - Case instruction

fn downstream_case_instr(
    renamer: &Renamer,
    ids_revive: &mut IdSet,
    instr_ol: CaseInstr,
) -> Result<InstrKind, StructureError> {
    let CaseInstr { exp, cases, total } = instr_ol;
    let exp = downstream_exp(renamer, ids_revive, exp);
    let cases = cases
        .into_iter()
        .map(|case| {
            let Case { guard, block } = case;
            let ids_used = underscores(guard.free());
            ids_revive.append(renamer.dom().intersection(&ids_used));
            let guard = renamer.rename_guard(guard);
            let block = downstream_block(renamer, ids_revive, block)?;
            let case = Case { guard, block };
            Ok(case)
        })
        .collect::<Result<_, StructureError>>()?;
    let instr = CaseInstr { exp, cases, total };
    Ok(InstrKind::Case(instr))
}

// - Group instruction

fn downstream_group_instr(
    renamer: &Renamer,
    ids_revive: &mut IdSet,
    instr_ol: GroupInstr,
) -> Result<InstrKind, StructureError> {
    let GroupInstr { id, rel_signature, exps, block } = instr_ol;
    let exps = downstream_exps(renamer, ids_revive, exps);
    let block = downstream_block(renamer, ids_revive, block)?;
    let instr = GroupInstr { id, rel_signature, exps, block };
    Ok(InstrKind::Group(instr))
}

// - Let instruction

fn downstream_let_instr(
    renamer: &Renamer,
    ids_revive: &mut IdSet,
    instr_ol: LetInstr,
) -> Result<InstrKind, StructureError> {
    let LetInstr { exp_l, exp_r, iter_instrs, block } = instr_ol;
    let exp_r = downstream_exp(renamer, ids_revive, exp_r);
    let iter_instrs = renamer.rename_iterinstrs_bound(iter_instrs);
    // The RHS uses the outer _x; the body uses the newly bound _x
    let ids_bound = underscores(exp_l.free());
    let renamer = renamer.filter(|id, _| !ids_bound.contains(id));
    let block = downstream_block(&renamer, ids_revive, block)?;
    let instr = LetInstr { exp_l, exp_r, iter_instrs, block };
    Ok(InstrKind::Let(instr))
}

// - Rule instruction

fn downstream_rule_instr(
    renamer: &Renamer,
    ids_revive: &mut IdSet,
    instr_ol: RuleInstr,
    span: &Span,
) -> Result<InstrKind, StructureError> {
    let RuleInstr { id, not_exp, input_hint, iter_instrs, block } = instr_ol;
    let exps = not_exp.args().into_iter().cloned().collect();
    let (exps_input, exps_output) = input::split(&input_hint, exps)
        .map_err(|error| StructureError::new(StructureErrorKind::Input(error), span.clone()))?;
    let exps_input = downstream_exps(renamer, ids_revive, exps_input);
    let ids_bound = underscores(exps_output.as_slice().free());
    let exps = input::combine(&input_hint, exps_input, exps_output)
        .map_err(|error| StructureError::new(StructureErrorKind::Input(error), span.clone()))?;
    let mixop = not_exp.to_mixop();
    let not_exp = Mixop::fill(&mixop, exps).expect("validated arguments preserve the mixfix arity");
    let iter_instrs = renamer.rename_iterinstrs_bound(iter_instrs);
    let renamer = renamer.filter(|id, _| !ids_bound.contains(id));
    let block = downstream_block(&renamer, ids_revive, block)?;
    let instr = RuleInstr { id, not_exp, input_hint, iter_instrs, block };
    Ok(InstrKind::Rule(instr))
}

// - Result instruction

fn downstream_result_instr(
    renamer: &Renamer,
    ids_revive: &mut IdSet,
    instr_ol: ResultInstr,
) -> Result<InstrKind, StructureError> {
    let ResultInstr { rel_signature, exps } = instr_ol;
    let exps = downstream_exps(renamer, ids_revive, exps);
    let instr = ResultInstr { rel_signature, exps };
    Ok(InstrKind::Result(instr))
}

// - Return instruction

fn downstream_return_instr(
    renamer: &Renamer,
    ids_revive: &mut IdSet,
    instr_ol: ReturnInstr,
) -> Result<InstrKind, StructureError> {
    let ReturnInstr { exp } = instr_ol;
    let exp = downstream_exp(renamer, ids_revive, exp);
    let instr = ReturnInstr { exp };
    Ok(InstrKind::Return(instr))
}

// - Debug instruction

fn downstream_debug_instr(
    renamer: &Renamer,
    ids_revive: &mut IdSet,
    instr_ol: DebugInstr,
) -> Result<InstrKind, StructureError> {
    let DebugInstr { exp, instr } = instr_ol;
    let exp = downstream_exp(renamer, ids_revive, exp);
    let instr = downstream_instr(renamer, ids_revive, *instr)?;
    let instr = Box::new(instr);
    let instr = DebugInstr { exp, instr };
    Ok(InstrKind::Debug(instr))
}

// == Upstream bindings

// Choose candidate names, let downstream rewrite their uses, then rename binders
// `let (_x, _y) = source { return _x }`
// becomes `let (x, _y) = source { return x }` when x is available

fn upstream_instr(frees: &IdSet, instr_ol: Instr) -> Result<Instr, StructureError> {
    let instr_kind_ol = upstream_instr_kind(frees, instr_ol.node, &instr_ol.span)?;
    let instr = crate::phrase!(node: instr_kind_ol, span: instr_ol.span);
    Ok(instr)
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

fn upstream_block(frees: &IdSet, block: Block) -> Result<Block, StructureError> {
    block
        .into_iter()
        .map(|instr_ol| upstream_instr(frees, instr_ol))
        .collect()
}

// - If instruction

fn upstream_if_instr(frees: &IdSet, instr_ol: IfInstr) -> Result<InstrKind, StructureError> {
    let IfInstr { exp, iter_exps, block } = instr_ol;
    let block = upstream_block(frees, block)?;
    let instr = IfInstr { exp, iter_exps, block };
    Ok(InstrKind::If(instr))
}

// - Hold instruction

fn upstream_hold_instr(frees: &IdSet, instr_ol: HoldInstr) -> Result<InstrKind, StructureError> {
    let HoldInstr { id, not_exp, iter_exps, block_hold, block_not_hold } = instr_ol;
    let block_hold = upstream_block(frees, block_hold)?;
    let block_not_hold = upstream_block(frees, block_not_hold)?;
    let instr = HoldInstr { id, not_exp, iter_exps, block_hold, block_not_hold };
    Ok(InstrKind::Hold(instr))
}

// - Case instruction

fn upstream_case_instr(frees: &IdSet, instr_ol: CaseInstr) -> Result<InstrKind, StructureError> {
    let CaseInstr { exp, cases, total } = instr_ol;
    let cases = cases
        .into_iter()
        .map(|case| {
            let Case { guard, block } = case;
            let block = upstream_block(frees, block)?;
            let case = Case { guard, block };
            Ok(case)
        })
        .collect::<Result<_, StructureError>>()?;
    let instr = CaseInstr { exp, cases, total };
    Ok(InstrKind::Case(instr))
}

// - Group instruction

fn upstream_group_instr(frees: &IdSet, instr_ol: GroupInstr) -> Result<InstrKind, StructureError> {
    let GroupInstr { id, rel_signature, exps, block } = instr_ol;
    let block = upstream_block(frees, block)?;
    let instr = GroupInstr { id, rel_signature, exps, block };
    Ok(InstrKind::Group(instr))
}

// - Let instruction

fn upstream_let_instr(frees: &IdSet, instr_ol: LetInstr) -> Result<InstrKind, StructureError> {
    let LetInstr { exp_l, exp_r, iter_instrs, block } = instr_ol;
    let ids_bound = underscores(exp_l.free());
    let (_, renamer) = candid_renamer(frees.clone(), &ids_bound);
    let mut ids_revive = IdSet::new();
    let block = downstream_block(&renamer, &mut ids_revive, block)?;
    let renamer = renamer.filter(|id, _| ids_revive.contains(id));
    let exp_l = renamer.rename_exp(exp_l);
    let iter_instrs = renamer.rename_iterinstrs_bind(iter_instrs);
    let block = renamer.rename_block(block)?;
    let instr = LetInstr { exp_l, exp_r, iter_instrs, block };
    Ok(InstrKind::Let(instr))
}

// - Rule instruction

fn upstream_rule_instr(
    frees: &IdSet,
    instr_ol: RuleInstr,
    span: &Span,
) -> Result<InstrKind, StructureError> {
    let RuleInstr { id, not_exp, input_hint, iter_instrs, block } = instr_ol;
    let exps = not_exp.args().into_iter().cloned().collect();
    let (exps_input, exps_output) = input::split(&input_hint, exps)
        .map_err(|error| StructureError::new(StructureErrorKind::Input(error), span.clone()))?;
    let ids_bound = underscores(exps_output.as_slice().free());
    let (_, renamer) = candid_renamer(frees.clone(), &ids_bound);
    let mut ids_revive = IdSet::new();
    let block = downstream_block(&renamer, &mut ids_revive, block)?;
    let renamer = renamer.filter(|id, _| ids_revive.contains(id));
    let exps_output = renamer.rename_exps(exps_output);
    let exps = input::combine(&input_hint, exps_input, exps_output)
        .map_err(|error| StructureError::new(StructureErrorKind::Input(error), span.clone()))?;
    let mixop = not_exp.to_mixop();
    let not_exp = Mixop::fill(&mixop, exps).expect("validated arguments preserve the mixfix arity");
    let iter_instrs = renamer.rename_iterinstrs_bind(iter_instrs);
    let instr = RuleInstr { id, not_exp, input_hint, iter_instrs, block };
    Ok(InstrKind::Rule(instr))
}

// == Entry points

// An input _x used only in `else { return _x }` is also renamed to x

pub(crate) fn apply_rel(
    (mut exps_match, mut block, mut block_else): (Vec<Exp>, Block, Option<Block>),
) -> Result<(Vec<Exp>, Block, Option<Block>), StructureError> {
    let frees_input = exps_match.as_slice().free();
    let ids_bound = underscores(frees_input.clone());
    let frees_else = block_else.as_ref().map(Free::free).unwrap_or_default();
    let frees = frees_input.union(block.free()).union(frees_else);
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
    let frees_else = block_else.as_ref().map(Free::free).unwrap_or_default();
    let frees = frees_input.union(block.free()).union(frees_else);
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
