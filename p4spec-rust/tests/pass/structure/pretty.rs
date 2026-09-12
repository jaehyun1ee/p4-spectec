#[path = "pretty/rename_tick.rs"]
mod rename_tick;
#[path = "pretty/revive_underscore.rs"]
mod revive_underscore;

use super::{id, instr, span, variable};
use crate::lang::il::ast::{Exp, ExpKind, Id, Iter, TypKind, Var};
use crate::pass::structure::ol::ast::*;
fn var_id(exp: &Exp) -> &Id {
    let ExpKind::Var(id) = &exp.node else {
        panic!("expected variable")
    };
    id
}
fn return_exp(instr_ol: &Instr) -> &Exp {
    let InstrKind::Return(instr_return) = &instr_ol.node else {
        panic!("expected return")
    };
    &instr_return.exp
}
fn binding(text_l: &str, text_r: &str, block: Block) -> Instr {
    instr(InstrKind::Let(LetInstr {
        exp_l: variable(text_l),
        exp_r: variable(text_r),
        iter_instrs: vec![],
        block,
    }))
}
fn iterator(text_bound: &str, text_bind: &str) -> InstrIter {
    let var = |text| Var {
        id: id(text),
        typ: crate::phrase! { node: TypKind::Bool, span: span(2) },
        iters: vec![],
    };
    InstrIter {
        iter: Iter::List,
        vars_bound: vec![var(text_bound)],
        vars_bind: vec![var(text_bind)],
    }
}
