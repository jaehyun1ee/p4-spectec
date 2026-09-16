//! Combine OL If and Case instructions into ordered case branches
//!
//! `casify_if_if` turns disjoint tests of the same value into a Case;
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
//! merging equal guards' bodies while preserving branch priority
//! Iterated Ifs and instructions other than If or Case stop the search

use std::collections::VecDeque;

use super::super::overlap::{Overlap, overlap_exp};
use crate::{
    lang::{
        common::source::{Phrase, Span},
        traits::eq::SyntaxEq,
    },
    pass::structure::{StructureError, ol::ast::*, opt::merge::merge_block},
    runtime::envs::algo::TDEnv,
};

use crate::pass::structure::{
    error::StructureErrorKind,
    opt::overlap::{exp_as_guard, overlap_guard},
};

// == [1] if-and-if to case analysis
fn casify_if_if(
    tdenv: &TDEnv,
    instr_target: &IfInstr,
    instr_if: &IfInstr,
) -> Result<Option<CaseInstr>, StructureError> {
    let overlap = overlap_exp(tdenv, &instr_target.exp, &instr_if.exp)?;
    match overlap {
        Overlap::Disjoint {
            exp,
            guard_a,
            guard_b,
        } => Ok(Some(case_from_ifs(
            exp,
            guard_a,
            guard_b,
            instr_target,
            instr_if,
            false,
        ))),
        Overlap::Partition {
            exp,
            guard_a,
            guard_b,
        } => Ok(Some(case_from_ifs(
            exp,
            guard_a,
            guard_b,
            instr_target,
            instr_if,
            true,
        ))),
        Overlap::Identical | Overlap::Fuzzy => Ok(None),
    }
}

fn case_from_ifs(
    exp: Exp,
    guard_a: Guard,
    guard_b: Guard,
    instr_target: &IfInstr,
    instr_if: &IfInstr,
    total: bool,
) -> CaseInstr {
    CaseInstr {
        exp,
        cases: vec![
            Case {
                guard: guard_a,
                block: instr_target.block.clone(),
            },
            Case {
                guard: guard_b,
                block: instr_if.block.clone(),
            },
        ],
        total,
    }
}

// == [2] if-and-case to case analysis
fn merge_if_case(
    tdenv: &TDEnv,
    instr_target: &IfInstr,
    instr_case: &CaseInstr,
    span_case: &Span,
) -> Result<Option<Vec<Case>>, StructureError> {
    let IfInstr {
        exp: exp_cond_target,
        block: block_target,
        ..
    } = instr_target;
    let CaseInstr { exp, cases, total } = instr_case;
    let Some(guard_target) = exp_as_guard(exp, exp_cond_target) else {
        return Ok(None);
    };
    for (idx, case) in cases.iter().enumerate() {
        let Case { guard, block } = case;
        match overlap_guard(tdenv, exp, &guard_target, guard)? {
            Overlap::Identical => {
                let mut cases = cases[idx..].to_vec();
                cases[0].block = merge_block(block_target.clone(), block.clone());
                return Ok(Some(cases));
            }
            Overlap::Disjoint { .. } | Overlap::Partition { .. } => {}
            Overlap::Fuzzy => return Ok(None),
        }
    }
    if *total {
        return Err(StructureError::new(
            StructureErrorKind::EmptyTotalCase,
            span_case.clone(),
        ));
    }
    let mut cases = cases.clone();
    cases.push(Case {
        guard: guard_target,
        block: block_target.clone(),
    });
    Ok(Some(cases))
}

fn casify_if_case(
    tdenv: &TDEnv,
    instr_target: &IfInstr,
    instr_case: &CaseInstr,
    span_case: &Span,
) -> Result<Option<CaseInstr>, StructureError> {
    Ok(
        merge_if_case(tdenv, instr_target, instr_case, span_case)?.map(|cases| CaseInstr {
            exp: instr_case.exp.clone(),
            cases,
            total: instr_case.total,
        }),
    )
}

// == [3] case-and-if to case analysis
fn merge_case_if(
    tdenv: &TDEnv,
    instr_target: &CaseInstr,
    instr_if: &IfInstr,
    span_target: &Span,
) -> Result<Option<Vec<Case>>, StructureError> {
    let CaseInstr { exp, cases, total } = instr_target;
    let IfInstr {
        exp: exp_cond,
        block,
        ..
    } = instr_if;
    let Some(guard) = exp_as_guard(exp, exp_cond) else {
        return Ok(None);
    };
    merge_case_guard(tdenv, exp, cases, *total, &guard, block, span_target)
}

fn merge_case_guard(
    tdenv: &TDEnv,
    exp_target: &Exp,
    cases_target: &[Case],
    total_target: bool,
    guard: &Guard,
    block: &Block,
    span_target: &Span,
) -> Result<Option<Vec<Case>>, StructureError> {
    for (idx, case_target) in cases_target.iter().enumerate() {
        let Case {
            guard: guard_target,
            block: block_target,
        } = case_target;
        match overlap_guard(tdenv, exp_target, guard_target, guard)? {
            Overlap::Identical => {
                let mut cases = cases_target[idx..].to_vec();
                cases[0].block = merge_block(block_target.clone(), block.clone());
                return Ok(Some(cases));
            }
            Overlap::Disjoint { .. } | Overlap::Partition { .. } => {}
            Overlap::Fuzzy => return Ok(None),
        }
    }
    if total_target {
        return Err(StructureError::new(
            StructureErrorKind::EmptyTotalCase,
            span_target.clone(),
        ));
    }
    let mut cases = cases_target.to_vec();
    cases.push(Case {
        guard: guard.clone(),
        block: block.clone(),
    });
    Ok(Some(cases))
}

fn casify_case_if(
    tdenv: &TDEnv,
    instr_target: &CaseInstr,
    instr_if: &IfInstr,
    span_target: &Span,
) -> Result<Option<CaseInstr>, StructureError> {
    Ok(
        merge_case_if(tdenv, instr_target, instr_if, span_target)?.map(|cases| CaseInstr {
            exp: instr_target.exp.clone(),
            cases,
            total: false,
        }),
    )
}

// == [4] case-and-case to case analysis
fn merge_case_case(
    tdenv: &TDEnv,
    instr_target: &CaseInstr,
    instr_case: &CaseInstr,
    span_target: &Span,
) -> Result<Option<Vec<Case>>, StructureError> {
    let CaseInstr {
        exp: exp_target,
        cases: cases_target,
        total: total_target,
    } = instr_target;
    let CaseInstr { exp, cases, .. } = instr_case;
    if !exp_target.syntax_eq(exp) {
        return Ok(None);
    }
    let mut cases_target = cases_target.clone();
    for case in cases {
        let Case { guard, block } = case;
        let Some(cases) = merge_case_guard(
            tdenv,
            exp_target,
            &cases_target,
            *total_target,
            guard,
            block,
            span_target,
        )?
        else {
            return Ok(None);
        };
        cases_target = cases;
    }
    Ok(Some(cases_target))
}

fn casify_case_case(
    tdenv: &TDEnv,
    instr_target: &CaseInstr,
    instr_case: &CaseInstr,
    span_target: &Span,
) -> Result<Option<CaseInstr>, StructureError> {
    Ok(
        merge_case_case(tdenv, instr_target, instr_case, span_target)?.map(|cases| CaseInstr {
            exp: instr_target.exp.clone(),
            cases,
            total: instr_target.total,
        }),
    )
}

// == [1/2] Casifying from an if statement
fn casify_from_if(
    tdenv: &TDEnv,
    instr_target: &IfInstr,
    instrs: &VecDeque<Instr>,
) -> Result<Option<(usize, CaseInstr)>, StructureError> {
    if !instr_target.iter_exps.is_empty() {
        return Ok(None);
    }
    for (idx, instr) in instrs.iter().enumerate() {
        let instr_kind = &instr.node;
        let instr_case = match instr_kind {
            InstrKind::If(instr_if) if instr_if.iter_exps.is_empty() => {
                casify_if_if(tdenv, instr_target, instr_if)?
            }
            InstrKind::Case(instr_case) => {
                casify_if_case(tdenv, instr_target, instr_case, &instr.span)?
            }
            _ => break,
        };
        if let Some(instr_case) = instr_case {
            return Ok(Some((idx, instr_case)));
        }
    }
    Ok(None)
}

// == [3/4] Casifying from a case statement
fn casify_from_case(
    tdenv: &TDEnv,
    instr_target: &CaseInstr,
    span_target: &Span,
    instrs: &VecDeque<Instr>,
) -> Result<Option<(usize, CaseInstr)>, StructureError> {
    for (idx, instr) in instrs.iter().enumerate() {
        let instr_kind = &instr.node;
        let instr_case = match instr_kind {
            InstrKind::If(instr_if) if instr_if.iter_exps.is_empty() => {
                casify_case_if(tdenv, instr_target, instr_if, span_target)?
            }
            InstrKind::Case(instr_case) => {
                casify_case_case(tdenv, instr_target, instr_case, span_target)?
            }
            _ => break,
        };
        if let Some(instr_case) = instr_case {
            return Ok(Some((idx, instr_case)));
        }
    }
    Ok(None)
}

fn casify_if_instr(
    tdenv: &TDEnv,
    instr_if: IfInstr,
    span: &Span,
    instrs: &mut VecDeque<Instr>,
) -> Result<InstrKind, StructureError> {
    if let Some((idx, instr_case)) = casify_from_if(tdenv, &instr_if, instrs)? {
        instrs.remove(idx);
        return casify_case_instr(tdenv, instr_case, span, instrs);
    }
    let IfInstr {
        exp,
        iter_exps,
        block,
    } = instr_if;
    let block = casify(tdenv, block)?;
    Ok(InstrKind::If(IfInstr {
        exp,
        iter_exps,
        block,
    }))
}

fn casify_case_instr(
    tdenv: &TDEnv,
    mut instr_case: CaseInstr,
    span: &Span,
    instrs: &mut VecDeque<Instr>,
) -> Result<InstrKind, StructureError> {
    while let Some((idx, instr_case_merged)) = casify_from_case(tdenv, &instr_case, span, instrs)? {
        instrs.remove(idx);
        instr_case = instr_case_merged;
    }
    let CaseInstr { exp, cases, total } = instr_case;
    let cases = cases
        .into_iter()
        .map(|case| {
            let Case { guard, block } = case;
            Ok(Case {
                guard,
                block: casify(tdenv, block)?,
            })
        })
        .collect::<Result<_, StructureError>>()?;
    Ok(InstrKind::Case(CaseInstr { exp, cases, total }))
}

fn casify_hold_instr(tdenv: &TDEnv, instr: HoldInstr) -> Result<InstrKind, StructureError> {
    let HoldInstr {
        id,
        not_exp,
        iter_exps,
        block_hold,
        block_not_hold,
    } = instr;
    let block_hold = casify(tdenv, block_hold)?;
    let block_not_hold = casify(tdenv, block_not_hold)?;
    Ok(InstrKind::Hold(HoldInstr {
        id,
        not_exp,
        iter_exps,
        block_hold,
        block_not_hold,
    }))
}

fn casify_group_instr(tdenv: &TDEnv, instr: GroupInstr) -> Result<InstrKind, StructureError> {
    let GroupInstr {
        id,
        rel_signature,
        exps,
        block,
    } = instr;
    let block = casify(tdenv, block)?;
    Ok(InstrKind::Group(GroupInstr {
        id,
        rel_signature,
        exps,
        block,
    }))
}

fn casify_let_instr(tdenv: &TDEnv, instr: LetInstr) -> Result<InstrKind, StructureError> {
    let LetInstr {
        exp_l,
        exp_r,
        iter_instrs,
        block,
    } = instr;
    let block = casify(tdenv, block)?;
    Ok(InstrKind::Let(LetInstr {
        exp_l,
        exp_r,
        iter_instrs,
        block,
    }))
}

fn casify_rule_instr(tdenv: &TDEnv, instr: RuleInstr) -> Result<InstrKind, StructureError> {
    let RuleInstr {
        id,
        not_exp,
        input_hint,
        iter_instrs,
        block,
    } = instr;
    let block = casify(tdenv, block)?;
    Ok(InstrKind::Rule(RuleInstr {
        id,
        not_exp,
        input_hint,
        iter_instrs,
        block,
    }))
}

fn casify(tdenv: &TDEnv, block: Block) -> Result<Block, StructureError> {
    let mut block_output = Vec::with_capacity(block.len());
    let mut instrs = VecDeque::from(block);
    while let Some(instr) = instrs.pop_front() {
        let Phrase {
            node: instr_kind,
            span,
            ..
        } = instr;
        let instr_kind = casify_instr_kind(tdenv, instr_kind, &span, &mut instrs)?;
        block_output.push(crate::phrase!(node: instr_kind, span: span));
    }
    Ok(block_output)
}

fn casify_instr_kind(
    tdenv: &TDEnv,
    instr_kind: InstrKind,
    span: &Span,
    instrs: &mut VecDeque<Instr>,
) -> Result<InstrKind, StructureError> {
    match instr_kind {
        InstrKind::If(instr) => casify_if_instr(tdenv, instr, span, instrs),
        InstrKind::Hold(instr) => casify_hold_instr(tdenv, instr),
        InstrKind::Case(instr) => casify_case_instr(tdenv, instr, span, instrs),
        InstrKind::Group(instr) => casify_group_instr(tdenv, instr),
        InstrKind::Let(instr) => casify_let_instr(tdenv, instr),
        InstrKind::Rule(instr) => casify_rule_instr(tdenv, instr),
        InstrKind::Return(_) | InstrKind::Result(_) | InstrKind::Debug(_) => Ok(instr_kind),
    }
}

pub(crate) fn apply(tdenv: &TDEnv, block: Block) -> Result<Block, StructureError> {
    casify(tdenv, block)
}
