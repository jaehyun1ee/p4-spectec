//! Mark variant case analyses whose guards cover exactly all constructors

use super::{
    error::{StructureError, StructureErrorKind},
    ol::ast::*,
    opt::overlap::typ_as_variant,
};
use crate::{
    lang::{
        common::source::Phrase,
        il::ast::{Mixop, Pattern},
    },
    runtime::envs::algo::TDEnv,
};
use std::collections::BTreeSet;

fn find_variant_case_analysis(
    tdenv: &TDEnv,
    cases: &[Case],
) -> Result<Option<Vec<Mixop>>, StructureError> {
    let mut mixops = Vec::new();
    for case in cases {
        match &case.guard {
            Guard::Sub(typ, _) => {
                let mixops_sub = typ_as_variant(tdenv, typ)?.ok_or_else(|| {
                    StructureError::new(
                        StructureErrorKind::NonVariantTotalization,
                        typ.span.clone(),
                    )
                })?;
                mixops.extend(mixops_sub);
            }
            Guard::Match(Pattern::Case(mixop)) => mixops.push(mixop.as_ref().clone()),
            _ => return Ok(None),
        }
    }
    Ok(Some(mixops))
}

fn totalize_case_analysis(tdenv: &TDEnv, block: Block) -> Result<Block, StructureError> {
    block
        .into_iter()
        .map(|instr| totalize_instr(tdenv, instr))
        .collect()
}

fn totalize_instr(tdenv: &TDEnv, instr: Instr) -> Result<Instr, StructureError> {
    let Phrase {
        node: instr_kind,
        span,
        ..
    } = instr;
    let instr_kind = totalize_instr_kind(tdenv, instr_kind)?;
    Ok(crate::phrase!(node: instr_kind, span: span))
}

fn totalize_instr_kind(tdenv: &TDEnv, instr_kind: InstrKind) -> Result<InstrKind, StructureError> {
    match instr_kind {
        InstrKind::If(instr) => totalize_if(tdenv, instr),
        InstrKind::Hold(instr) => totalize_hold(tdenv, instr),
        InstrKind::Case(instr) => totalize_case(tdenv, instr),
        InstrKind::Group(instr) => totalize_group(tdenv, instr),
        InstrKind::Let(instr) => totalize_let(tdenv, instr),
        InstrKind::Rule(instr) => totalize_rule(tdenv, instr),
        // The source deliberately leaves Debug and its enclosed instruction alone
        instr_kind => Ok(instr_kind),
    }
}

fn totalize_if(tdenv: &TDEnv, instr: IfInstr) -> Result<InstrKind, StructureError> {
    let IfInstr {
        exp,
        iter_exps,
        block,
    } = instr;
    let block = totalize_case_analysis(tdenv, block)?;
    Ok(InstrKind::If(IfInstr {
        exp,
        iter_exps,
        block,
    }))
}
fn totalize_hold(tdenv: &TDEnv, instr: HoldInstr) -> Result<InstrKind, StructureError> {
    let HoldInstr {
        id,
        not_exp,
        iter_exps,
        block_hold,
        block_not_hold,
    } = instr;
    let block_hold = totalize_case_analysis(tdenv, block_hold)?;
    let block_not_hold = totalize_case_analysis(tdenv, block_not_hold)?;
    Ok(InstrKind::Hold(HoldInstr {
        id,
        not_exp,
        iter_exps,
        block_hold,
        block_not_hold,
    }))
}
fn totalize_case(tdenv: &TDEnv, instr: CaseInstr) -> Result<InstrKind, StructureError> {
    let CaseInstr { exp, cases, total } = instr;
    let cases = cases
        .into_iter()
        .map(|case| totalize_case_block(tdenv, case))
        .collect::<Result<Vec<_>, _>>()?;
    let total = if let Some(mixops_case) = find_variant_case_analysis(tdenv, &cases)? {
        let typ = crate::phrase!(node: exp.note.as_ref().clone(), span: exp.span.clone());
        let mixops_total = typ_as_variant(tdenv, &typ)?.ok_or_else(|| {
            StructureError::new(StructureErrorKind::NonVariantTotalization, typ.span.clone())
        })?;
        let mixops_total: BTreeSet<_> = mixops_total.into_iter().collect();
        let mixops_case: BTreeSet<_> = mixops_case.into_iter().collect();
        mixops_case == mixops_total
    } else {
        total
    };
    Ok(InstrKind::Case(CaseInstr { exp, cases, total }))
}
fn totalize_case_block(tdenv: &TDEnv, case: Case) -> Result<Case, StructureError> {
    let Case { guard, block } = case;
    let block = totalize_case_analysis(tdenv, block)?;
    Ok(Case { guard, block })
}
fn totalize_group(tdenv: &TDEnv, instr: GroupInstr) -> Result<InstrKind, StructureError> {
    let GroupInstr {
        id,
        rel_signature,
        exps,
        block,
    } = instr;
    let block = totalize_case_analysis(tdenv, block)?;
    Ok(InstrKind::Group(GroupInstr {
        id,
        rel_signature,
        exps,
        block,
    }))
}
fn totalize_let(tdenv: &TDEnv, instr: LetInstr) -> Result<InstrKind, StructureError> {
    let LetInstr {
        exp_l,
        exp_r,
        iter_instrs,
        block,
    } = instr;
    let block = totalize_case_analysis(tdenv, block)?;
    Ok(InstrKind::Let(LetInstr {
        exp_l,
        exp_r,
        iter_instrs,
        block,
    }))
}
fn totalize_rule(tdenv: &TDEnv, instr: RuleInstr) -> Result<InstrKind, StructureError> {
    let RuleInstr {
        id,
        not_exp,
        input_hint,
        iter_instrs,
        block,
    } = instr;
    let block = totalize_case_analysis(tdenv, block)?;
    Ok(InstrKind::Rule(RuleInstr {
        id,
        not_exp,
        input_hint,
        iter_instrs,
        block,
    }))
}

#[derive(Debug)]
pub(crate) struct Blocks {
    pub block: Block,
    pub block_else: Option<Block>,
}

pub(crate) fn totalize(
    tdenv: &TDEnv,
    block: Block,
    block_else: Option<Block>,
) -> Result<Blocks, StructureError> {
    let block = totalize_case_analysis(tdenv, block)?;
    let block_else = block_else
        .map(|block| totalize_case_analysis(tdenv, block))
        .transpose()?;
    Ok(Blocks { block, block_else })
}
pub(crate) fn totalize_without_else(tdenv: &TDEnv, block: Block) -> Result<Block, StructureError> {
    totalize_case_analysis(tdenv, block)
}
