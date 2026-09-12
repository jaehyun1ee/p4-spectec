//! Remove matches whose expanded variant type has exactly one constructor
use crate::pass::structure::{StructureError, ol::ast::*, opt::overlap::typ_as_variant};
use crate::{
    lang::{
        common::source::{NotePhrase, Span},
        il::ast::{ExpKind, Typ},
    },
    runtime::envs::algo::TDEnv,
};
fn is_singleton_case(tdenv: &TDEnv, typ: &Typ) -> Result<bool, StructureError> {
    Ok(typ_as_variant(tdenv, typ)?.is_some_and(|mixops| mixops.len() == 1))
}
fn is_singleton_match(tdenv: &TDEnv, exp: &Exp) -> Result<bool, StructureError> {
    match &exp.node {
        ExpKind::Match(exp, _) => is_singleton_match_exp(tdenv, exp),
        _ => Ok(false),
    }
}
fn is_singleton_match_exp(tdenv: &TDEnv, exp: &Exp) -> Result<bool, StructureError> {
    let typ = crate::phrase!(node: exp.note.as_ref().clone(), span: exp.span.clone());
    is_singleton_case(tdenv, &typ)
}
fn remove_instr(instr_ol: Instr, tdenv: &TDEnv) -> Result<Block, StructureError> {
    let NotePhrase {
        node: instr_kind_ol,
        note: (),
        span,
    } = instr_ol;
    remove_instr_kind(tdenv, instr_kind_ol, span)
}
fn remove_instr_kind(
    tdenv: &TDEnv,
    instr_kind_ol: InstrKind,
    span: Span,
) -> Result<Block, StructureError> {
    match instr_kind_ol {
        InstrKind::If(instr_ol) => remove_if_instr(tdenv, instr_ol, span),
        InstrKind::Hold(instr_ol) => remove_hold_instr(tdenv, instr_ol, span),
        InstrKind::Case(instr_ol) => remove_case_instr(tdenv, instr_ol, span),
        InstrKind::Group(instr_ol) => remove_group_instr(tdenv, instr_ol, span),
        InstrKind::Let(instr_ol) => remove_let_instr(tdenv, instr_ol, span),
        InstrKind::Rule(instr_ol) => remove_rule_instr(tdenv, instr_ol, span),
        InstrKind::Result(_) | InstrKind::Return(_) | InstrKind::Debug(_) => {
            Ok(vec![crate::phrase!(node: instr_kind_ol, span: span)])
        }
    }
}
fn remove_if_instr(tdenv: &TDEnv, instr_ol: IfInstr, span: Span) -> Result<Block, StructureError> {
    let IfInstr {
        exp,
        iter_exps,
        block,
    } = instr_ol;
    if is_singleton_match(tdenv, &exp)? {
        return remove_block(tdenv, block);
    }
    let block = remove_block(tdenv, block)?;
    Ok(vec![
        crate::phrase!(node: InstrKind::If(IfInstr { exp, iter_exps, block }), span: span),
    ])
}
fn remove_hold_instr(
    tdenv: &TDEnv,
    instr_ol: HoldInstr,
    span: Span,
) -> Result<Block, StructureError> {
    let HoldInstr {
        id,
        not_exp,
        iter_exps,
        block_hold,
        block_not_hold,
    } = instr_ol;
    let block_hold = remove_block(tdenv, block_hold)?;
    let block_not_hold = remove_block(tdenv, block_not_hold)?;
    Ok(vec![
        crate::phrase!(node: InstrKind::Hold(HoldInstr { id, not_exp, iter_exps, block_hold, block_not_hold }), span: span),
    ])
}
fn remove_case_instr(
    tdenv: &TDEnv,
    instr_ol: CaseInstr,
    span: Span,
) -> Result<Block, StructureError> {
    let CaseInstr { exp, cases, total } = instr_ol;
    let cases = cases
        .into_iter()
        .map(|case| {
            let Case { guard, block } = case;
            let block = remove_block(tdenv, block)?;
            Ok(Case { guard, block })
        })
        .collect::<Result<_, StructureError>>()?;
    Ok(vec![
        crate::phrase!(node: InstrKind::Case(CaseInstr { exp, cases, total }), span: span),
    ])
}
fn remove_group_instr(
    tdenv: &TDEnv,
    instr_ol: GroupInstr,
    span: Span,
) -> Result<Block, StructureError> {
    let GroupInstr {
        id,
        rel_signature,
        exps,
        block,
    } = instr_ol;
    let block = remove_block(tdenv, block)?;
    Ok(vec![
        crate::phrase!(node: InstrKind::Group(GroupInstr { id, rel_signature, exps, block }), span: span),
    ])
}
fn remove_let_instr(
    tdenv: &TDEnv,
    instr_ol: LetInstr,
    span: Span,
) -> Result<Block, StructureError> {
    let LetInstr {
        exp_l,
        exp_r,
        iter_instrs,
        block,
    } = instr_ol;
    let block = remove_block(tdenv, block)?;
    Ok(vec![
        crate::phrase!(node: InstrKind::Let(LetInstr { exp_l, exp_r, iter_instrs, block }), span: span),
    ])
}
fn remove_rule_instr(
    tdenv: &TDEnv,
    instr_ol: RuleInstr,
    span: Span,
) -> Result<Block, StructureError> {
    let RuleInstr {
        id,
        not_exp,
        input_hint,
        iter_instrs,
        block,
    } = instr_ol;
    let block = remove_block(tdenv, block)?;
    Ok(vec![
        crate::phrase!(node: InstrKind::Rule(RuleInstr { id, not_exp, input_hint, iter_instrs, block }), span: span),
    ])
}
fn remove_block(tdenv: &TDEnv, block: Block) -> Result<Block, StructureError> {
    let mut block_rewritten = Vec::new();
    for instr_ol in block {
        block_rewritten.extend(remove_instr(instr_ol, tdenv)?);
    }
    Ok(block_rewritten)
}
pub(crate) fn apply(tdenv: &TDEnv, block: Block) -> Result<Block, StructureError> {
    remove_block(tdenv, block)
}
