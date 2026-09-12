use super::super::{id, instr, ret, signature, span, variable};
use crate::{
    lang::{
        common::notation::mixfix::Mixfix,
        hints::input::InputHint,
        il::ast::{DefTypKind, ExpKind, Iter, Pattern, Typ, TypKind},
    },
    pass::structure::ol::ast::*,
};
fn literal() -> Exp {
    crate::note_phrase!(node: ExpKind::Bool(true), note: TypKind::Bool, span: span(3))
}
fn binding(exp_r: Exp, block: Block) -> Instr {
    instr(InstrKind::Let(LetInstr {
        exp_l: variable("x"),
        exp_r,
        iter_instrs: vec![],
        block,
    }))
}
fn rule(block: Block, input_hint: InputHint) -> Instr {
    instr(InstrKind::Rule(RuleInstr {
        id: id("R"),
        not_exp: Mixfix::Seq(vec![
            Mixfix::Arg(variable("input")),
            Mixfix::Arg(variable("x")),
        ]),
        input_hint,
        iter_instrs: vec![],
        block,
    }))
}
#[path = "post/remove_let_dead.rs"]
mod remove_let_dead;
#[path = "post/remove_match_singleton.rs"]
mod remove_match_singleton;
