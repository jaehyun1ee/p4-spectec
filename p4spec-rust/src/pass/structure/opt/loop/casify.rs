//! Combine OL If and Case instructions into ordered case branches
//!
//! `casify_if_then_if` turns disjoint tests of the same value into a Case;
//! complementary tests also mark the Case as total:
//!
//! ```text
//! if x = true { return a }; if x = false { return b }
//!
//! becomes
//!
//! case x (total) { true => return a; false => return b }
//! ```
//!
//! Tests `x = 1` and `x = 2` on an integer form a partial Case instead
//! `casify_from_if` and `casify_from_case` also combine existing Cases,
//! then `casify_block` retries the combined Case before entering its bodies
//! When an existing Case has an equal guard, the scan drops preceding cases:
//! `if x = 2 { A }; case x { 1 => B; 2 => C; 3 => D }`
//! becomes `case x { 2 => A; C; 3 => D }`, matching the OCaml scan
//! Iterated Ifs and instructions other than If or Case stop the search

use std::collections::VecDeque;

use crate::{
    lang::{common::source::Span, traits::eq::SyntaxEq},
    pass::structure::{
        StructureError, StructureErrorKind,
        ol::ast::*,
        opt::{
            merge::merge_block,
            overlap::{Overlap, exp_as_guard, overlap_exp, overlap_guard},
        },
    },
    runtime::envs::algo::TDEnv,
};

// == Instructions

fn casify_block(tdenv: &TDEnv, changed: &mut bool, block: Block) -> Result<Block, StructureError> {
    let mut block_output = Vec::with_capacity(block.len());
    let mut instrs = VecDeque::from(block);
    while let Some(instr) = instrs.pop_front() {
        let instr_kind = casify_instr_kind(tdenv, changed, &instr.span, &mut instrs, instr.node)?;
        let instr = crate::phrase!(node: instr_kind, span: instr.span);
        block_output.push(instr);
    }
    Ok(block_output)
}

fn casify_instr_kind(
    tdenv: &TDEnv,
    changed: &mut bool,
    span: &Span,
    instrs: &mut VecDeque<Instr>,
    instr_kind: InstrKind,
) -> Result<InstrKind, StructureError> {
    match instr_kind {
        InstrKind::If(instr) => casify_if_instr(tdenv, changed, span, instrs, instr),
        InstrKind::Hold(instr) => casify_hold_instr(tdenv, changed, instr),
        InstrKind::Case(instr) => casify_case_instr(tdenv, changed, span, instrs, instr),
        InstrKind::Group(instr) => casify_group_instr(tdenv, changed, instr),
        InstrKind::Let(instr) => casify_let_instr(tdenv, changed, instr),
        InstrKind::Rule(instr) => casify_rule_instr(tdenv, changed, instr),
        InstrKind::Return(_) | InstrKind::Result(_) | InstrKind::Debug(_) => Ok(instr_kind),
    }
}

// - If instruction

fn casify_if_instr(
    tdenv: &TDEnv,
    changed: &mut bool,
    span: &Span,
    instrs: &mut VecDeque<Instr>,
    mut instr_if: IfInstr,
) -> Result<InstrKind, StructureError> {
    if let Some((idx, instr_case)) = casify_from_if(tdenv, &mut instr_if, instrs)? {
        *changed = true;
        instrs.remove(idx);
        return casify_case_instr(tdenv, changed, span, instrs, instr_case);
    }
    let IfInstr { exp, iter_exps, block } = instr_if;
    let block = casify_block(tdenv, changed, block)?;
    let instr = IfInstr { exp, iter_exps, block };
    Ok(InstrKind::If(instr))
}

// - Hold instruction

fn casify_hold_instr(
    tdenv: &TDEnv,
    changed: &mut bool,
    instr: HoldInstr,
) -> Result<InstrKind, StructureError> {
    let HoldInstr { id, not_exp, iter_exps, block_hold, block_not_hold } = instr;
    let block_hold = casify_block(tdenv, changed, block_hold)?;
    let block_not_hold = casify_block(tdenv, changed, block_not_hold)?;
    let instr = HoldInstr { id, not_exp, iter_exps, block_hold, block_not_hold };
    Ok(InstrKind::Hold(instr))
}

// - Case instruction

fn casify_case_instr(
    tdenv: &TDEnv,
    changed: &mut bool,
    span: &Span,
    instrs: &mut VecDeque<Instr>,
    mut instr_case: CaseInstr,
) -> Result<InstrKind, StructureError> {
    while let Some((idx, instr_case_merged)) =
        casify_from_case(tdenv, &mut instr_case, span, instrs)?
    {
        *changed = true;
        instrs.remove(idx);
        instr_case = instr_case_merged;
    }
    let CaseInstr { exp, cases, total } = instr_case;
    let cases = cases
        .into_iter()
        .map(|case| {
            let Case { guard, block } = case;
            let block = casify_block(tdenv, changed, block)?;
            let case = Case { guard, block };
            Ok(case)
        })
        .collect::<Result<_, StructureError>>()?;
    let instr = CaseInstr { exp, cases, total };
    Ok(InstrKind::Case(instr))
}

// - Group instruction

fn casify_group_instr(
    tdenv: &TDEnv,
    changed: &mut bool,
    instr: GroupInstr,
) -> Result<InstrKind, StructureError> {
    let GroupInstr { id, rel_signature, exps, block } = instr;
    let block = casify_block(tdenv, changed, block)?;
    let instr = GroupInstr { id, rel_signature, exps, block };
    Ok(InstrKind::Group(instr))
}

// - Let instruction

fn casify_let_instr(
    tdenv: &TDEnv,
    changed: &mut bool,
    instr: LetInstr,
) -> Result<InstrKind, StructureError> {
    let LetInstr { exp_l, exp_r, iter_instrs, block } = instr;
    let block = casify_block(tdenv, changed, block)?;
    let instr = LetInstr { exp_l, exp_r, iter_instrs, block };
    Ok(InstrKind::Let(instr))
}

// - Rule instruction

fn casify_rule_instr(
    tdenv: &TDEnv,
    changed: &mut bool,
    instr: RuleInstr,
) -> Result<InstrKind, StructureError> {
    let RuleInstr { id, not_exp, input_hint, iter_instrs, block } = instr;
    let block = casify_block(tdenv, changed, block)?;
    let instr = RuleInstr { id, not_exp, input_hint, iter_instrs, block };
    Ok(InstrKind::Rule(instr))
}

// == Downstream search

// Keep skipped Ifs/Cases in place and return the first successful combination

fn casify_from_if(
    tdenv: &TDEnv,
    instr_target: &mut IfInstr,
    instrs: &mut VecDeque<Instr>,
) -> Result<Option<(usize, CaseInstr)>, StructureError> {
    if !instr_target.iter_exps.is_empty() {
        return Ok(None);
    }
    for (idx, instr) in instrs.iter_mut().enumerate() {
        let instr_case = match &mut instr.node {
            InstrKind::If(instr_if) if instr_if.iter_exps.is_empty() => {
                casify_if_then_if(tdenv, instr_target, instr_if)?
            }
            InstrKind::Case(instr_case) => {
                casify_if_then_case(tdenv, instr_target, instr_case, &instr.span)?
            }
            _ => break,
        };
        if let Some(instr_case) = instr_case {
            return Ok(Some((idx, instr_case)));
        }
    }
    Ok(None)
}

fn casify_from_case(
    tdenv: &TDEnv,
    instr_target: &mut CaseInstr,
    span_target: &Span,
    instrs: &mut VecDeque<Instr>,
) -> Result<Option<(usize, CaseInstr)>, StructureError> {
    for (idx, instr) in instrs.iter_mut().enumerate() {
        let instr_case = match &mut instr.node {
            InstrKind::If(instr_if) if instr_if.iter_exps.is_empty() => {
                casify_case_then_if(tdenv, instr_target, instr_if, span_target)?
            }
            InstrKind::Case(instr_case) => {
                casify_case_then_case(tdenv, instr_target, instr_case, span_target)?
            }
            _ => break,
        };
        if let Some(instr_case) = instr_case {
            return Ok(Some((idx, instr_case)));
        }
    }
    Ok(None)
}

// == Combining conditions

// - If and If

fn casify_if_then_if(
    tdenv: &TDEnv,
    instr_target: &mut IfInstr,
    instr_if: &mut IfInstr,
) -> Result<Option<CaseInstr>, StructureError> {
    let overlap = overlap_exp(tdenv, &instr_target.exp, &instr_if.exp)?;
    let (exp, guard_a, guard_b, total) = match overlap {
        // x = 1 and x = 2 leave other integer values uncovered
        Overlap::Disjoint { exp, guard_a, guard_b } => (exp, guard_a, guard_b, false),
        // x = true and x = false cover both boolean values
        Overlap::Partition { exp, guard_a, guard_b } => (exp, guard_a, guard_b, true),
        Overlap::Identical | Overlap::Fuzzy => return Ok(None),
    };
    let block_a = std::mem::take(&mut instr_target.block);
    let case_a = Case { guard: guard_a, block: block_a };
    let block_b = std::mem::take(&mut instr_if.block);
    let case_b = Case { guard: guard_b, block: block_b };
    let cases = vec![case_a, case_b];
    let instr = CaseInstr { exp, cases, total };
    Ok(Some(instr))
}

// - If and Case

fn casify_if_then_case(
    tdenv: &TDEnv,
    instr_target: &mut IfInstr,
    instr_case: &mut CaseInstr,
    span_case: &Span,
) -> Result<Option<CaseInstr>, StructureError> {
    let IfInstr { exp: exp_cond_target, block: block_target, .. } = instr_target;
    let CaseInstr { exp, cases, total } = instr_case;
    let Some(guard_target) = exp_as_guard(exp, exp_cond_target) else {
        return Ok(None);
    };
    for (idx, case) in cases.iter_mut().enumerate() {
        let Case { guard, block } = case;
        let overlap = overlap_guard(tdenv, exp, &guard_target, guard)?;
        match overlap {
            Overlap::Identical => {
                // if x = 2 before cases [1, 2, 3] keeps cases [2, 3]
                let block_target = std::mem::take(block_target);
                let block = std::mem::take(block);
                let block = merge_block(block_target, block);
                let mut cases = std::mem::take(cases);
                cases.drain(..idx);
                cases[0].block = block;
                let instr = CaseInstr { exp: exp.clone(), cases, total: *total };
                return Ok(Some(instr));
            }
            Overlap::Disjoint { .. } | Overlap::Partition { .. } => {}
            Overlap::Fuzzy => return Ok(None),
        }
    }
    if *total {
        return Err(StructureError::new(StructureErrorKind::EmptyTotalCase, span_case.clone()));
    }
    let mut cases = std::mem::take(cases);
    let block = std::mem::take(block_target);
    let case = Case { guard: guard_target, block };
    cases.push(case);
    let instr = CaseInstr { exp: exp.clone(), cases, total: *total };
    Ok(Some(instr))
}

// - Case and If

fn casify_case_then_if(
    tdenv: &TDEnv,
    instr_target: &mut CaseInstr,
    instr_if: &mut IfInstr,
    span_target: &Span,
) -> Result<Option<CaseInstr>, StructureError> {
    let CaseInstr { exp, cases, total } = instr_target;
    let IfInstr { exp: exp_cond, block, .. } = instr_if;
    let Some(guard) = exp_as_guard(exp, exp_cond) else {
        return Ok(None);
    };
    let Some(idx) = find_case_merge(
        tdenv,
        exp,
        cases.iter().map(|case| &case.guard),
        *total,
        &guard,
        span_target,
    )?
    else {
        return Ok(None);
    };
    let mut cases = std::mem::take(cases);
    let block = std::mem::take(block);
    let case = Case { guard, block };
    apply_case_merge(idx, case, &mut cases);
    // Case followed by If becomes partial, even when the Case was total
    let instr = CaseInstr { exp: exp.clone(), cases, total: false };
    Ok(Some(instr))
}

// - Case and Case

fn casify_case_then_case(
    tdenv: &TDEnv,
    instr_target: &mut CaseInstr,
    instr_case: &mut CaseInstr,
    span_target: &Span,
) -> Result<Option<CaseInstr>, StructureError> {
    let CaseInstr { exp: exp_target, cases: cases_target, total: total_target } = instr_target;
    let CaseInstr { exp, cases, .. } = instr_case;
    if !exp_target.syntax_eq(exp) {
        return Ok(None);
    }
    // A later fuzzy guard must leave both input bodies untouched
    let mut guards: VecDeque<_> = cases_target.iter().map(|case| &case.guard).collect();
    let mut idxs = Vec::with_capacity(cases.len());
    for case in cases.iter() {
        let Some(idx) = find_case_merge(
            tdenv,
            exp_target,
            guards.iter().copied(),
            *total_target,
            &case.guard,
            span_target,
        )?
        else {
            return Ok(None);
        };
        if idx == guards.len() {
            guards.push_back(&case.guard);
        } else {
            guards.drain(..idx);
        }
        idxs.push(idx);
    }
    let mut cases_target = std::mem::take(cases_target);
    let cases = std::mem::take(cases);
    for (case, idx) in cases.into_iter().zip(idxs) {
        apply_case_merge(idx, case, &mut cases_target);
    }
    let instr = CaseInstr { exp: exp_target.clone(), cases: cases_target, total: *total_target };
    Ok(Some(instr))
}

// - Guard analysis and owned body merging

// The index identifies an equal guard, or the end position for appending
fn find_case_merge<'a>(
    tdenv: &TDEnv,
    exp_target: &Exp,
    guards_target: impl ExactSizeIterator<Item = &'a Guard>,
    total_target: bool,
    guard: &Guard,
    span_target: &Span,
) -> Result<Option<usize>, StructureError> {
    let guards_len = guards_target.len();
    for (idx, guard_target) in guards_target.enumerate() {
        match overlap_guard(tdenv, exp_target, guard_target, guard)? {
            Overlap::Identical => return Ok(Some(idx)),
            Overlap::Disjoint { .. } | Overlap::Partition { .. } => {}
            Overlap::Fuzzy => return Ok(None),
        }
    }
    if total_target {
        return Err(StructureError::new(StructureErrorKind::EmptyTotalCase, span_target.clone()));
    }
    Ok(Some(guards_len))
}

fn apply_case_merge(idx: usize, case: Case, cases: &mut Vec<Case>) {
    if idx == cases.len() {
        cases.push(case);
    } else {
        // Cases [1, 2, 3] followed by guard 2 keep cases [2, 3]
        cases.drain(..idx);
        let block_target = std::mem::take(&mut cases[0].block);
        cases[0].block = merge_block(block_target, case.block);
    }
}

// == Entry point

pub(crate) fn apply(
    tdenv: &TDEnv,
    changed: &mut bool,
    block: Block,
) -> Result<Block, StructureError> {
    casify_block(tdenv, changed, block)
}
