use super::super::*;
use crate::lang::{common::ds::set::IdSet, traits::free::Free};
#[test]
fn test_let_patterns_and_both_hold_blocks_are_collected() {
    let instr_let = instr(ast_ol::InstrKind::Let(ast_ol::LetInstr {
        exp_l: variable("pattern"),
        exp_r: variable("source"),
        iter_instrs: vec![InstrIter {
            iter: Iter::List,
            vars_bound: vec![Var {
                id: id("metadata"),
                typ: crate::phrase! {node: TypKind::Bool, span: span(1)},
                iters: vec![],
            }],
            vars_bind: vec![],
        }],
        block: vec![ret("body")],
    }));
    let instr_hold = instr(ast_ol::InstrKind::Hold(ast_ol::HoldInstr {
        id: id("relation"),
        not_exp: Mixfix::Arg(variable("argument")),
        iter_exps: vec![],
        block_hold: vec![instr_let],
        block_not_hold: vec![ret("fallback")],
    }));
    let ids: IdSet = ["pattern", "source", "body", "argument", "fallback"]
        .into_iter()
        .map(id)
        .collect();
    assert_eq!(instr_hold.free(), ids);
}
