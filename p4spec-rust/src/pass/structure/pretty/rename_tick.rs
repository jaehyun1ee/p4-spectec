//! Shorten tick suffixes on definition inputs and OL binding names
//!
//! `find_rename_ticks` picks the smallest unused tick count for each base
//! name; `upstream_block` carries enclosing names into nested bindings
//! When `x` is already in use but `x'` is available:
//!
//! ```text
//! let x''' = source { return (x''', x) }
//!
//! becomes
//!
//! let x' = source { return (x', x) }
//! ```
//!
//! `apply_rel` and `apply_func` rename definition inputs consistently in
//! both the main body and fallback, avoiding names already used there

use crate::lang::{
    common::{ds::set::IdSet, source::Span},
    hints::input,
    il::ast::{Arg, Exp, Mixop},
    traits::free::Free,
};
use crate::pass::structure::{
    StructureError, StructureErrorKind, ol::ast::*, re::renamer::Renamer,
};

// == Candidate names

// With {x, x'', x'''}, x''' becomes x'; its own spelling does not reserve a slot
fn find_rename_ticks(frees: &IdSet, id: &Id) -> Option<Id> {
    let mut id_rename = strip_trailing_ticks(id);
    let counts: Vec<_> = frees
        .iter()
        .filter(|id_free| {
            id_free.node != id.node && strip_trailing_ticks(id_free).node == id_rename.node
        })
        .map(count_trailing_ticks)
        .collect();
    let mut count = 0;
    while counts.contains(&count) {
        count += 1;
    }
    id_rename.node.push_str(&"'".repeat(count));
    (id.node != id_rename.node).then_some(id_rename)
}

fn strip_trailing_ticks(id: &Id) -> Id {
    let mut id_strip = id.clone();
    id_strip
        .node
        .truncate(id.node.len() - count_trailing_ticks(id));
    id_strip
}

fn count_trailing_ticks(id: &Id) -> usize {
    id.node
        .bytes()
        .rev()
        .take_while(|byte| *byte == b'\'')
        .count()
}

// Reserve each choice before the next binding: (x'', x''') can become (x, x')
fn binding_renamer(mut frees: IdSet, ids: &IdSet) -> Renamer {
    let mut renamer = Renamer::empty();
    for id in ids.iter() {
        if let Some(id_rename) = find_rename_ticks(&frees, id) {
            frees.take(id);
            frees.insert(id_rename.clone());
            renamer.add(id.clone(), id_rename);
        } else {
            frees.insert(id.clone());
        }
    }
    renamer
}

// == Upstream bindings

// Carry enclosing names into each body so new names cannot capture them
// Under `if x { ... }`, a binding x' can stay x' but cannot become x

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
    let frees = exp.free().union(frees.clone());
    let block = upstream_block(&frees, block)?;
    let instr = IfInstr { exp, iter_exps, block };
    Ok(InstrKind::If(instr))
}

// - Hold instruction

fn upstream_hold_instr(frees: &IdSet, instr_ol: HoldInstr) -> Result<InstrKind, StructureError> {
    let HoldInstr { id, not_exp, iter_exps, block_hold, block_not_hold } = instr_ol;
    let frees = not_exp.free().union(frees.clone());
    let block_hold = upstream_block(&frees, block_hold)?;
    let block_not_hold = upstream_block(&frees, block_not_hold)?;
    let instr = HoldInstr { id, not_exp, iter_exps, block_hold, block_not_hold };
    Ok(InstrKind::Hold(instr))
}

// - Case instruction

fn upstream_case_instr(frees: &IdSet, instr_ol: CaseInstr) -> Result<InstrKind, StructureError> {
    let CaseInstr { exp, cases, total } = instr_ol;
    let frees = exp.free().union(frees.clone());
    let cases = cases
        .into_iter()
        .map(|case| {
            let Case { guard, block } = case;
            let frees = guard.free().union(frees.clone());
            let block = upstream_block(&frees, block)?;
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
    let frees = exps.as_slice().free().union(frees.clone());
    let block = upstream_block(&frees, block)?;
    let instr = GroupInstr { id, rel_signature, exps, block };
    Ok(InstrKind::Group(instr))
}

// - Let instruction

fn upstream_let_instr(
    frees_upstream: &IdSet,
    instr_ol: LetInstr,
) -> Result<InstrKind, StructureError> {
    let LetInstr { exp_l, exp_r, iter_instrs, block } = instr_ol;
    let frees_l = exp_l.free();
    let frees = frees_l
        .clone()
        .union(exp_r.free())
        .union(block.free())
        .union(frees_upstream.clone());
    let renamer = binding_renamer(frees, &frees_l);
    let exp_l = renamer.rename_exp(exp_l);
    let iter_instrs = renamer.rename_iterinstrs_bind(iter_instrs);
    let frees = exp_l
        .free()
        .union(exp_r.free())
        .union(frees_upstream.clone());
    let block = renamer.rename_block(block)?;
    let block = upstream_block(&frees, block)?;
    let instr = LetInstr { exp_l, exp_r, iter_instrs, block };
    Ok(InstrKind::Let(instr))
}

// - Rule instruction

fn upstream_rule_instr(
    frees_upstream: &IdSet,
    instr_ol: RuleInstr,
    span: &Span,
) -> Result<InstrKind, StructureError> {
    let RuleInstr { id, not_exp, input_hint, iter_instrs, block } = instr_ol;
    let exps = not_exp.args().into_iter().cloned().collect();
    let (exps_input, exps_output) = input::split(&input_hint, exps)
        .map_err(|error| StructureError::new(StructureErrorKind::Input(error), span.clone()))?;
    let frees_output = exps_output.as_slice().free();
    let frees = exps_input
        .as_slice()
        .free()
        .union(frees_output.clone())
        .union(block.free())
        .union(frees_upstream.clone());
    let renamer = binding_renamer(frees, &frees_output);
    let exps_output = renamer.rename_exps(exps_output);
    let iter_instrs = renamer.rename_iterinstrs_bind(iter_instrs);
    let frees = exps_input
        .as_slice()
        .free()
        .union(exps_output.as_slice().free())
        .union(frees_upstream.clone());
    let exps = input::combine(&input_hint, exps_input, exps_output)
        .map_err(|error| StructureError::new(StructureErrorKind::Input(error), span.clone()))?;
    let mixop = not_exp.to_mixop();
    let not_exp = Mixop::fill(&mixop, exps).expect("validated arguments preserve the mixfix arity");
    let block = renamer.rename_block(block)?;
    let block = upstream_block(&frees, block)?;
    let instr = RuleInstr { id, not_exp, input_hint, iter_instrs, block };
    Ok(InstrKind::Rule(instr))
}

// == Definition inputs

fn upstream_exps(
    (mut exps_match, mut block, mut block_else): (Vec<Exp>, Block, Option<Block>),
) -> Result<(Vec<Exp>, Block, Option<Block>), StructureError> {
    let ids = exps_match.as_slice().free();
    let frees_else = block_else.as_ref().map(Free::free).unwrap_or_default();
    let mut frees = ids.clone().union(block.free()).union(frees_else);
    for id in ids.iter() {
        if let Some(id_rename) = find_rename_ticks(&frees, id) {
            frees.take(id);
            frees.insert(id_rename.clone());
            let renamer = Renamer::singleton(id.clone(), id_rename);
            exps_match = renamer.rename_exps(exps_match);
            block = renamer.rename_block(block)?;
            block_else = block_else
                .map(|block| renamer.rename_block(block))
                .transpose()?;
        }
    }
    Ok((exps_match, block, block_else))
}

fn upstream_args(
    (mut args_input, mut block, mut block_else): (Vec<Arg>, Block, Option<Block>),
) -> Result<(Vec<Arg>, Block, Option<Block>), StructureError> {
    let ids = args_input.as_slice().free();
    let frees_else = block_else.as_ref().map(Free::free).unwrap_or_default();
    let mut frees = ids.clone().union(block.free()).union(frees_else);
    for id in ids.iter() {
        if let Some(id_rename) = find_rename_ticks(&frees, id) {
            frees.take(id);
            frees.insert(id_rename.clone());
            let renamer = Renamer::singleton(id.clone(), id_rename);
            args_input = renamer.rename_args(args_input);
            block = renamer.rename_block(block)?;
            block_else = block_else
                .map(|block| renamer.rename_block(block))
                .transpose()?;
        }
    }
    Ok((args_input, block, block_else))
}

// == Entry points

pub(crate) fn apply_rel(
    body: (Vec<Exp>, Block, Option<Block>),
) -> Result<(Vec<Exp>, Block, Option<Block>), StructureError> {
    let (exps_match, mut block, mut block_else) = upstream_exps(body)?;
    let frees = exps_match.as_slice().free();
    block = upstream_block(&frees, block)?;
    block_else = block_else
        .map(|block| upstream_block(&frees, block))
        .transpose()?;
    Ok((exps_match, block, block_else))
}

pub(crate) fn apply_func(
    body: (Vec<Arg>, Block, Option<Block>),
) -> Result<(Vec<Arg>, Block, Option<Block>), StructureError> {
    let (args_input, mut block, mut block_else) = upstream_args(body)?;
    let frees = args_input.as_slice().free();
    block = upstream_block(&frees, block)?;
    block_else = block_else
        .map(|block| upstream_block(&frees, block))
        .transpose()?;
    Ok((args_input, block, block_else))
}
