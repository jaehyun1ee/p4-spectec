use super::super::{id, id_exp, instr, ret, signature, span};
use crate::lang::il::ast::{ExpKind, Iter, TypKind};
use crate::pass::structure::ol::ast::*;
fn group(block: Block) -> Instr {
    instr(InstrKind::Group(GroupInstr {
        id: id("group"),
        rel_signature: signature(),
        exps: vec![id_exp("x")],
        block,
    }))
}

fn binding(exp_l: Exp, exp_r: Exp, block: Block) -> Instr {
    instr(InstrKind::Let(LetInstr { exp_l, exp_r, iter_instrs: vec![], block }))
}

fn iterated(text: &str, iter: Iter) -> Exp {
    crate::note_phrase! {node: ExpKind::Iter(Box::new(id_exp(text)), (iter, vec![])), note: TypKind::Bool, span: span(4)}
}

#[path = "pre/matchify_if_eq_terminal.rs"]
mod matchify_if_eq_terminal;
#[path = "pre/remove_group.rs"]
mod remove_group;
#[path = "pre/remove_let_alias.rs"]
mod remove_let_alias;

fn containers(block: Block) -> Block {
    use crate::lang::{common::notation::mixfix::Mixfix, hints::input::InputHint};
    let exp_literal =
        crate::note_phrase! {node: ExpKind::Bool(true), note: TypKind::Bool, span: span(3)};
    let iter_instr = InstrIter {
        iter: Iter::List,
        vars_bound: vec![crate::lang::il::ast::Var {
            id: id("bound"),
            typ: crate::phrase! {node: TypKind::Bool, span: span(3)},
            iters: vec![],
        }],
        vars_bind: vec![crate::lang::il::ast::Var {
            id: id("bind"),
            typ: crate::phrase! {node: TypKind::Bool, span: span(4)},
            iters: vec![],
        }],
    };
    vec![
        instr(InstrKind::If(IfInstr {
            exp: id_exp("condition"),
            iter_exps: vec![(Iter::Opt, vec![])],
            block: block.clone(),
        })),
        instr(InstrKind::Hold(HoldInstr {
            id: id("relation"),
            not_exp: Mixfix::Arg(id_exp("input")),
            iter_exps: vec![(Iter::List, vec![])],
            block_hold: block.clone(),
            block_not_hold: block.clone(),
        })),
        instr(InstrKind::Case(CaseInstr {
            exp: id_exp("case"),
            cases: vec![
                Case { guard: Guard::Bool(true), block: block.clone() },
                Case { guard: Guard::Bool(false), block: block.clone() },
            ],
            total: true,
        })),
        instr(InstrKind::Let(LetInstr {
            exp_l: id_exp("binding"),
            exp_r: exp_literal,
            iter_instrs: vec![iter_instr.clone()],
            block: block.clone(),
        })),
        instr(InstrKind::Rule(RuleInstr {
            id: id("relation"),
            not_exp: Mixfix::Arg(id_exp("input")),
            input_hint: InputHint::new(vec![0]),
            iter_instrs: vec![iter_instr],
            block,
        })),
    ]
    .into_iter()
    .map(|mut instr_ol| {
        instr_ol.span = span(7);
        instr_ol
    })
    .collect()
}
