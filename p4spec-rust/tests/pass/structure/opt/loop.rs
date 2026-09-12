use super::super::{id, instr, ret, span, variable};
use crate::lang::{
    common::notation::mixfix::Mixfix,
    hints::input::InputHint,
    il::ast::{ExpKind, Iter, TypKind, Var},
};
use crate::pass::structure::ol::ast::*;
fn binding(text: &str, block: Block) -> Instr {
    instr(InstrKind::Let(LetInstr {
        exp_l: variable(text),
        exp_r: variable("input"),
        iter_instrs: vec![],
        block,
    }))
}
fn rule(text: &str, block: Block) -> Instr {
    instr(InstrKind::Rule(RuleInstr {
        id: id("relation"),
        not_exp: Mixfix::Seq(vec![
            Mixfix::Arg(variable("input")),
            Mixfix::Arg(variable(text)),
        ]),
        input_hint: InputHint::new(vec![0]),
        iter_instrs: vec![],
        block,
    }))
}
fn hold(block_hold: Block, block_not_hold: Block) -> Instr {
    instr(InstrKind::Hold(HoldInstr {
        id: id("relation"),
        not_exp: Mixfix::Arg(variable("input")),
        iter_exps: vec![],
        block_hold,
        block_not_hold,
    }))
}
fn var(text: &str) -> Var {
    Var {
        id: id(text),
        typ: crate::phrase! {node:TypKind::Bool,span:span(2)},
        iters: vec![],
    }
}
#[path = "loop/merge_binding.rs"]
mod merge_binding;
#[path = "loop/merge_hold.rs"]
mod merge_hold;

#[path = "loop/casify.rs"]
mod casify;
#[path = "loop/merge_if.rs"]
mod merge_if;
