use crate::lang::common::source::{NotePhrase, Span};
use crate::pass::structure::ol::ast::*;

fn remove_instr(instr_ol: Instr) -> Block {
    let NotePhrase {
        node: instr_kind_ol,
        note: (),
        span,
    } = instr_ol;
    remove_instr_kind(instr_kind_ol, span)
}
fn remove_instr_kind(instr_kind_ol: InstrKind, span: Span) -> Block {
    match instr_kind_ol {
        InstrKind::If(instr_ol) => remove_if_instr(instr_ol, span),
        InstrKind::Hold(instr_ol) => remove_hold_instr(instr_ol, span),
        InstrKind::Case(instr_ol) => remove_case_instr(instr_ol, span),
        InstrKind::Group(instr_ol) => remove_group_instr(instr_ol),
        InstrKind::Let(instr_ol) => remove_let_instr(instr_ol, span),
        InstrKind::Rule(instr_ol) => remove_rule_instr(instr_ol, span),
        InstrKind::Result(_) | InstrKind::Return(_) | InstrKind::Debug(_) => {
            vec![crate::phrase! {node: instr_kind_ol, span: span}]
        }
    }
}
fn remove_if_instr(instr_ol: IfInstr, span: Span) -> Block {
    let IfInstr {
        exp,
        iter_exps,
        block,
    } = instr_ol;
    let block = remove_block(block);
    vec![crate::phrase! {node: InstrKind::If(IfInstr {exp, iter_exps, block}), span: span}]
}
fn remove_hold_instr(instr_ol: HoldInstr, span: Span) -> Block {
    let HoldInstr {
        id,
        not_exp,
        iter_exps,
        block_hold,
        block_not_hold,
    } = instr_ol;
    let block_hold = remove_block(block_hold);
    let block_not_hold = remove_block(block_not_hold);
    vec![
        crate::phrase! {node: InstrKind::Hold(HoldInstr {id, not_exp, iter_exps, block_hold, block_not_hold}), span: span},
    ]
}
fn remove_case_instr(instr_ol: CaseInstr, span: Span) -> Block {
    let CaseInstr { exp, cases, total } = instr_ol;
    let cases = cases
        .into_iter()
        .map(|case| {
            let Case { guard, block } = case;
            let block = remove_block(block);
            Case { guard, block }
        })
        .collect::<Vec<_>>();
    vec![crate::phrase! {node: InstrKind::Case(CaseInstr {exp, cases, total}), span: span}]
}
fn remove_group_instr(instr_ol: GroupInstr) -> Block {
    let GroupInstr { block, .. } = instr_ol;
    remove_block(block)
}
fn remove_let_instr(instr_ol: LetInstr, span: Span) -> Block {
    let LetInstr {
        exp_l,
        exp_r,
        iter_instrs,
        block,
    } = instr_ol;
    let block = remove_block(block);
    vec![
        crate::phrase! {node: InstrKind::Let(LetInstr {exp_l, exp_r, iter_instrs, block}), span: span},
    ]
}
fn remove_rule_instr(instr_ol: RuleInstr, span: Span) -> Block {
    let RuleInstr {
        id,
        not_exp,
        input_hint,
        iter_instrs,
        block,
    } = instr_ol;
    let block = remove_block(block);
    vec![
        crate::phrase! {node: InstrKind::Rule(RuleInstr {id, not_exp, input_hint, iter_instrs, block}), span: span},
    ]
}
fn remove_block(block: Block) -> Block {
    block.into_iter().flat_map(remove_instr).collect()
}
pub(crate) fn apply(block: Block) -> Block {
    remove_block(block)
}
