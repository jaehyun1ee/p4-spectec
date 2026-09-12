use super::{id, instr, ret, signature, span, variable};
use crate::lang::{
    common::notation::mixfix::Mixfix,
    il::ast::{ExpKind, TypKind},
};
use crate::{
    lang::traits::eq::SyntaxEq,
    pass::structure::{
        ol::ast::*,
        opt::r#loop::{casify, merge_binding, merge_hold, merge_if},
        optimize::*,
    },
    runtime::envs::algo::TDEnv,
};
fn group(block: Block) -> Instr {
    instr(InstrKind::Group(GroupInstr {
        id: id("G"),
        rel_signature: signature(),
        exps: vec![variable("input")],
        block,
    }))
}
#[test]
fn test_group_modes_and_independent_else_optimization() {
    let tdenv = TDEnv::new();
    for without_rule_groups in [true, false] {
        let blocks = optimize_with_else(
            &tdenv,
            vec![group(vec![ret("main")])],
            Some(vec![group(vec![ret("else")])]),
            without_rule_groups,
        )
        .unwrap();
        let block_main = if without_rule_groups {
            vec![ret("main")]
        } else {
            vec![group(vec![ret("main")])]
        };
        let block_else = if without_rule_groups {
            vec![ret("else")]
        } else {
            vec![group(vec![ret("else")])]
        };
        assert_eq!(blocks.block, block_main);
        assert_eq!(blocks.block_else, Some(block_else));
        assert_eq!(
            optimize_without_else(&tdenv, vec![group(vec![ret("main")])], without_rule_groups)
                .unwrap(),
            block_main
        );
    }
    assert_eq!(
        optimize_with_else(&tdenv, vec![], Some(vec![]), true)
            .unwrap()
            .block_else,
        Some(vec![])
    );
    assert!(
        optimize_with_else(&tdenv, vec![], None, true)
            .unwrap()
            .block_else
            .is_none()
    );
}
#[test]
fn test_hold_merge_exposes_bindings_for_a_later_loop_iteration() {
    let tdenv = TDEnv::new();
    let binding = |text| {
        instr(InstrKind::Let(LetInstr {
            exp_l: variable("x"),
            exp_r: crate::note_phrase!(node: ExpKind::Bool(true), note: TypKind::Bool, span: span(3)),
            iter_instrs: vec![],
            block: vec![ret(text)],
        }))
    };
    let hold = |block| {
        instr(InstrKind::Hold(HoldInstr {
            id: id("R"),
            not_exp: Mixfix::Arg(variable("input")),
            iter_exps: vec![],
            block_hold: block,
            block_not_hold: vec![],
        }))
    };
    let block = vec![hold(vec![binding("x")]), hold(vec![binding("x")])];
    let block_once = merge_binding::apply(block.clone()).unwrap();
    let block_once = merge_if::apply(&tdenv, block_once).unwrap();
    let block_once = merge_hold::apply(block_once);
    let block_once = casify::apply(&tdenv, block_once).unwrap();
    let block = optimize(&tdenv, block, true).unwrap();
    assert!(!block.syntax_eq(&block_once));
    let mut instr_expect = binding("x");
    let InstrKind::Let(instr_let) = &mut instr_expect.node else {
        panic!()
    };
    instr_let.block.push(ret("x"));
    assert_eq!(block, vec![hold(vec![instr_expect])]);
    assert!(
        optimize(&tdenv, block.clone(), true)
            .unwrap()
            .syntax_eq(&block)
    );
}

#[test]
fn test_post_liveness_runs_once_after_the_rewrite_loop() {
    let exp_literal =
        crate::note_phrase!(node: ExpKind::Bool(true), note: TypKind::Bool, span: span(3));
    let exp_tuple = crate::note_phrase!(node: ExpKind::Tuple(vec![variable("x")]), note: TypKind::Tuple(vec![]), span: span(4));
    let instr_inner = instr(InstrKind::Let(LetInstr {
        exp_l: variable("y"),
        exp_r: exp_tuple,
        iter_instrs: vec![],
        block: vec![],
    }));
    let instr_outer = |block| {
        instr(InstrKind::Let(LetInstr {
            exp_l: variable("x"),
            exp_r: exp_literal.clone(),
            iter_instrs: vec![],
            block,
        }))
    };
    let block = optimize(&TDEnv::new(), vec![instr_outer(vec![instr_inner])], true).unwrap();
    assert_eq!(block, vec![instr_outer(vec![])]);
}
