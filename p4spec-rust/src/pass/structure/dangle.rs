//! Preserve semantic fallthrough when lowering optimized instructions to SL

use super::{
    error::{StructureError, StructureErrorKind},
    ol::ast as ol,
};
use crate::lang::{
    common::source::{Phrase, Span},
    sl::ast as sl,
};

fn insert_dangle(block_ol: ol::Block) -> Result<sl::Block, StructureError> {
    insert_block(block_ol, true)
}

fn insert_block(block_ol: ol::Block, dangle: bool) -> Result<sl::Block, StructureError> {
    block_ol
        .into_iter()
        .map(|instr_ol| insert_instr(instr_ol, dangle))
        .collect()
}

fn insert_instr(instr_ol: ol::Instr, dangle: bool) -> Result<sl::Instr, StructureError> {
    let Phrase {
        node: instr_kind_ol,
        span,
        ..
    } = instr_ol;
    let instr_kind_sl = insert_instr_kind(instr_kind_ol, dangle, &span)?;
    Ok(crate::phrase!(node: instr_kind_sl, span: span))
}

fn insert_instr_kind(
    instr_kind_ol: ol::InstrKind,
    dangle: bool,
    span: &Span,
) -> Result<sl::InstrKind, StructureError> {
    match instr_kind_ol {
        ol::InstrKind::If(instr_ol) => insert_if(instr_ol, dangle),
        ol::InstrKind::Hold(instr_ol) => insert_hold(instr_ol, dangle, span),
        ol::InstrKind::Case(instr_ol) => insert_case(instr_ol, dangle),
        ol::InstrKind::Group(instr_ol) => insert_group(instr_ol, dangle),
        ol::InstrKind::Let(instr_ol) => insert_let(instr_ol, dangle),
        ol::InstrKind::Rule(instr_ol) => insert_rule(instr_ol, dangle),
        ol::InstrKind::Result(instr_ol) => Ok(insert_result(instr_ol)),
        ol::InstrKind::Return(instr_ol) => Ok(insert_return(instr_ol)),
        ol::InstrKind::Debug(instr_ol) => insert_debug(instr_ol, dangle),
    }
}

fn insert_if(instr_ol: ol::IfInstr, dangle: bool) -> Result<sl::InstrKind, StructureError> {
    let ol::IfInstr {
        exp,
        iter_exps,
        block: block_ol,
    } = instr_ol;
    let block = insert_block(block_ol, dangle)?;
    Ok(sl::InstrKind::If(sl::IfInstr {
        exp,
        iter_exps,
        block,
        dangle,
    }))
}
fn insert_hold(
    instr_ol: ol::HoldInstr,
    dangle: bool,
    span: &Span,
) -> Result<sl::InstrKind, StructureError> {
    let ol::HoldInstr {
        id,
        not_exp,
        iter_exps,
        block_hold: block_hold_ol,
        block_not_hold: block_not_hold_ol,
    } = instr_ol;
    let block_hold_sl = insert_block(block_hold_ol, dangle)?;
    let block_not_hold_sl = insert_block(block_not_hold_ol, dangle)?;
    let hold_case = match (block_hold_sl.is_empty(), block_not_hold_sl.is_empty()) {
        (true, true) => {
            return Err(StructureError::new(
                StructureErrorKind::EmptyHold,
                span.clone(),
            ));
        }
        (false, true) => sl::HoldCase::Hold(block_hold_sl, dangle),
        (true, false) => sl::HoldCase::NotHold(block_not_hold_sl, dangle),
        (false, false) => sl::HoldCase::Both(block_hold_sl, block_not_hold_sl),
    };
    Ok(sl::InstrKind::Hold(sl::HoldInstr {
        id,
        not_exp,
        iter_exps,
        hold_case,
    }))
}
fn insert_case(instr_ol: ol::CaseInstr, dangle: bool) -> Result<sl::InstrKind, StructureError> {
    let ol::CaseInstr {
        exp,
        cases: cases_ol,
        total,
    } = instr_ol;
    let cases = cases_ol
        .into_iter()
        .map(|case_ol| insert_case_block(case_ol, dangle))
        .collect::<Result<_, _>>()?;
    Ok(sl::InstrKind::Case(sl::CaseInstr {
        exp,
        cases,
        dangle: dangle && !total,
    }))
}
fn insert_case_block(case_ol: ol::Case, dangle: bool) -> Result<sl::Case, StructureError> {
    let ol::Case {
        guard,
        block: block_ol,
    } = case_ol;
    let block = insert_block(block_ol, dangle)?;
    Ok(sl::Case { guard, block })
}
fn insert_group(instr_ol: ol::GroupInstr, dangle: bool) -> Result<sl::InstrKind, StructureError> {
    let ol::GroupInstr {
        id,
        rel_signature,
        exps,
        block: block_ol,
    } = instr_ol;
    let block = insert_block(block_ol, dangle)?;
    Ok(sl::InstrKind::Group(sl::GroupInstr {
        id,
        rel_signature,
        exps,
        block,
    }))
}
fn insert_let(instr_ol: ol::LetInstr, dangle: bool) -> Result<sl::InstrKind, StructureError> {
    let ol::LetInstr {
        exp_l,
        exp_r,
        iter_instrs,
        block: block_ol,
    } = instr_ol;
    let block = insert_block(block_ol, dangle)?;
    Ok(sl::InstrKind::Let(sl::LetInstr {
        exp_l,
        exp_r,
        iter_instrs,
        block,
    }))
}
fn insert_rule(instr_ol: ol::RuleInstr, dangle: bool) -> Result<sl::InstrKind, StructureError> {
    let ol::RuleInstr {
        id,
        not_exp,
        input_hint,
        iter_instrs,
        block: block_ol,
    } = instr_ol;
    let block = insert_block(block_ol, dangle)?;
    Ok(sl::InstrKind::Rule(sl::RuleInstr {
        id,
        not_exp,
        input_hint,
        iter_instrs,
        block,
    }))
}
fn insert_result(instr_ol: ol::ResultInstr) -> sl::InstrKind {
    let ol::ResultInstr {
        rel_signature,
        exps,
    } = instr_ol;
    sl::InstrKind::Result(sl::ResultInstr {
        rel_signature,
        exps,
    })
}
fn insert_return(instr_ol: ol::ReturnInstr) -> sl::InstrKind {
    let ol::ReturnInstr { exp } = instr_ol;
    sl::InstrKind::Return(sl::ReturnInstr { exp })
}
fn insert_debug(instr_ol: ol::DebugInstr, dangle: bool) -> Result<sl::InstrKind, StructureError> {
    let ol::DebugInstr {
        exp,
        instr: instr_ol,
    } = instr_ol;
    let instr_sl = insert_instr(*instr_ol, dangle)?;
    Ok(sl::InstrKind::Debug(sl::DebugInstr {
        exp,
        instr: Box::new(instr_sl),
    }))
}

fn insert_nothing(block_ol: ol::Block) -> Result<sl::Block, StructureError> {
    insert_block(block_ol, false)
}

#[derive(Debug)]
pub(crate) struct Blocks {
    pub block: sl::Block,
    pub block_else: Option<sl::Block>,
}

pub(crate) fn instrument(
    block_ol: ol::Block,
    block_else_ol: Option<ol::Block>,
) -> Result<Blocks, StructureError> {
    match block_else_ol {
        Some(block_else_ol) => {
            let block = insert_nothing(block_ol)?;
            let block_else = insert_nothing(block_else_ol)?;
            Ok(Blocks {
                block,
                block_else: Some(block_else),
            })
        }
        None => {
            let block = insert_dangle(block_ol)?;
            Ok(Blocks {
                block,
                block_else: None,
            })
        }
    }
}

pub(crate) fn instrument_without_else(block_ol: ol::Block) -> Result<sl::Block, StructureError> {
    insert_nothing(block_ol)
}
