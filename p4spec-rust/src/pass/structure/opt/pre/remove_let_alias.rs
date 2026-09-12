use crate::lang::common::source::{NotePhrase, Span};
use crate::lang::{
    il::ast::{ExpKind, Id, Iter},
    traits::eq::SyntaxEq,
};
use crate::pass::structure::ol::ast::*;
use crate::pass::structure::{
    StructureError,
    re::{renamer::Renamer, replacer::Replacer},
};

fn remove_instr(instr_ol: Instr) -> Result<Block, StructureError> {
    let NotePhrase {
        node: instr_kind_ol,
        note: (),
        span,
    } = instr_ol;
    remove_instr_kind(instr_kind_ol, span)
}
fn remove_instr_kind(instr_kind_ol: InstrKind, span: Span) -> Result<Block, StructureError> {
    match instr_kind_ol {
        InstrKind::If(instr_ol) => remove_if_instr(instr_ol, span),
        InstrKind::Hold(instr_ol) => remove_hold_instr(instr_ol, span),
        InstrKind::Case(instr_ol) => remove_case_instr(instr_ol, span),
        InstrKind::Group(instr_ol) => remove_group_instr(instr_ol, span),
        InstrKind::Let(instr_ol) => remove_let_instr(instr_ol, span),
        InstrKind::Rule(instr_ol) => remove_rule_instr(instr_ol, span),
        InstrKind::Result(_) | InstrKind::Return(_) | InstrKind::Debug(_) => {
            Ok(vec![crate::phrase! {node: instr_kind_ol, span: span}])
        }
    }
}
fn remove_if_instr(instr_ol: IfInstr, span: Span) -> Result<Block, StructureError> {
    let IfInstr {
        exp,
        iter_exps,
        block,
    } = instr_ol;
    let block = remove_block(block)?;
    Ok(vec![
        crate::phrase! {node: InstrKind::If(IfInstr {exp, iter_exps, block}), span: span},
    ])
}
fn remove_hold_instr(instr_ol: HoldInstr, span: Span) -> Result<Block, StructureError> {
    let HoldInstr {
        id,
        not_exp,
        iter_exps,
        block_hold,
        block_not_hold,
    } = instr_ol;
    let block_hold = remove_block(block_hold)?;
    let block_not_hold = remove_block(block_not_hold)?;
    Ok(vec![
        crate::phrase! {node: InstrKind::Hold(HoldInstr {id, not_exp, iter_exps, block_hold, block_not_hold}), span: span},
    ])
}
fn remove_case_instr(instr_ol: CaseInstr, span: Span) -> Result<Block, StructureError> {
    let CaseInstr { exp, cases, total } = instr_ol;
    let cases = cases
        .into_iter()
        .map(|case| {
            let Case { guard, block } = case;
            let block = remove_block(block)?;
            Ok(Case { guard, block })
        })
        .collect::<Result<_, StructureError>>()?;
    Ok(vec![
        crate::phrase! {node: InstrKind::Case(CaseInstr {exp, cases, total}), span: span},
    ])
}
fn remove_group_instr(instr_ol: GroupInstr, span: Span) -> Result<Block, StructureError> {
    let GroupInstr {
        id,
        rel_signature,
        exps,
        block,
    } = instr_ol;
    let block = remove_block(block)?;
    Ok(vec![
        crate::phrase! {node: InstrKind::Group(GroupInstr {id, rel_signature, exps, block}), span: span},
    ])
}
fn remove_let_instr(instr_ol: LetInstr, span: Span) -> Result<Block, StructureError> {
    let LetInstr {
        exp_l,
        exp_r,
        iter_instrs,
        block,
    } = instr_ol;
    if let (ExpKind::Var(id_l), ExpKind::Var(id_r)) = (&exp_l.node, &exp_r.node) {
        let renamer = Renamer::singleton(id_l.clone(), id_r.clone());
        let block = renamer.rename_instrs(block)?;
        return remove_block(block);
    }
    if let (Some((id_l, iter_l)), Some((id_r, iter_r))) =
        (iterated_var(&exp_l), iterated_var(&exp_r))
        && iter_l.syntax_eq(iter_r)
    {
        let renamer = Renamer::singleton(id_l.clone(), id_r.clone());
        let block = renamer.rename_instrs(block)?;
        return remove_block(block);
    }
    if let ExpKind::Var(id_l) = &exp_l.node
        && iterated_var(&exp_r).is_some()
    {
        let replacer = Replacer::singleton(id_l.clone(), exp_r);
        let block = replacer.replace_instrs(block)?;
        return remove_block(block);
    }
    let block = remove_block(block)?;
    Ok(vec![
        crate::phrase! {node: InstrKind::Let(LetInstr {exp_l, exp_r, iter_instrs, block}), span: span},
    ])
}
fn remove_rule_instr(instr_ol: RuleInstr, span: Span) -> Result<Block, StructureError> {
    let RuleInstr {
        id,
        not_exp,
        input_hint,
        iter_instrs,
        block,
    } = instr_ol;
    let block = remove_block(block)?;
    Ok(vec![
        crate::phrase! {node: InstrKind::Rule(RuleInstr {id, not_exp, input_hint, iter_instrs, block}), span: span},
    ])
}
fn iterated_var(exp: &Exp) -> Option<(&Id, &Iter)> {
    let ExpKind::Iter(exp, (iter, _)) = &exp.node else {
        return None;
    };
    let ExpKind::Var(id) = &exp.node else {
        return None;
    };
    Some((id, iter))
}
fn remove_block(block: Block) -> Result<Block, StructureError> {
    let mut block_output = Vec::new();
    for instr_ol in block {
        let block = remove_instr(instr_ol)?;
        block_output.extend(block);
    }
    Ok(block_output)
}
pub(crate) fn apply(block: Block) -> Result<Block, StructureError> {
    remove_block(block)
}
