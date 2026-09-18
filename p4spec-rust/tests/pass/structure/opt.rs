#[path = "opt/loop.rs"]
mod loop_rewrites;
#[path = "opt/merge.rs"]
mod merge;
#[path = "opt/overlap.rs"]
mod overlap;
#[path = "opt/post.rs"]
mod post;
#[path = "opt/pre.rs"]
mod pre;

use super::{id, instr, ret, signature, span, variable};
use crate::lang::{
    common::notation::mixfix::Mixfix,
    il::ast::{ExpKind, TypKind},
};
use crate::{
    lang::traits::eq::SyntaxEq,
    pass::structure::{
        ol::ast::*,
        opt::{
            r#loop::{casify, merge_binding, merge_hold, merge_if},
            optimize,
        },
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
fn test_group_modes_preserve_or_remove_group_instructions() {
    let tdenv = TDEnv::new();
    for without_rule_groups in [true, false] {
        let block = vec![group(vec![ret("main")])];
        let block = optimize(&tdenv, block, without_rule_groups).unwrap();
        let block_expect =
            if without_rule_groups { vec![ret("main")] } else { vec![group(vec![ret("main")])] };
        assert_eq!(block, block_expect);
    }
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
    let block_once = merge_binding::apply(&mut false, block.clone()).unwrap();
    let block_once = merge_if::apply(&tdenv, &mut false, block_once).unwrap();
    let block_once = merge_hold::apply(&mut false, block_once);
    let block_once = casify::apply(&tdenv, &mut false, block_once).unwrap();
    let block = optimize(&tdenv, block, true).unwrap();
    assert!(!block.syntax_eq(&block_once));
    let mut instr_expect = binding("x");
    let InstrKind::Let(instr_let) = &mut instr_expect.node else { panic!() };
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

#[test]
fn test_fixed_point_moves_surviving_expression_payloads() {
    let hold = |block| {
        let instr_hold = HoldInstr {
            id: id("relation"),
            not_exp: Mixfix::Arg(variable("input")),
            iter_exps: vec![],
            block_hold: block,
            block_not_hold: vec![],
        };
        instr(InstrKind::Hold(instr_hold))
    };
    let instr_return = ret("payload");
    let InstrKind::Return(instr_body) = &instr_return.node else { unreachable!() };
    let ExpKind::Id(id_body) = &instr_body.exp.node else { unreachable!() };
    let ptr_body = id_body.node.as_ptr();
    let block = vec![hold(vec![instr_return]), hold(vec![ret("tail")])];
    let block = optimize(&TDEnv::new(), block, true).unwrap();
    let InstrKind::Hold(instr_hold) = &block[0].node else { panic!("expected hold") };
    let InstrKind::Return(instr_body) = &instr_hold.block_hold[0].node else { unreachable!() };
    let ExpKind::Id(id_body) = &instr_body.exp.node else { unreachable!() };
    assert_eq!(id_body.node.as_ptr(), ptr_body);
}

#[test]
fn test_reversed_equality_preserves_reachable_case_prefix() {
    use crate::lang::{
        il::ast::{CmpOp, OpTyp},
        xl::bool::CmpOp as BoolCmpOp,
    };
    let exp_target = crate::note_phrase!(node: crate::lang::il::ast::ExpKind::Id(id("x")), note: TypKind::Text, span: span(1));
    let literal = |text: &str| crate::note_phrase!(node: ExpKind::Text(text.into()), note: TypKind::Text, span: span(1));
    let equality = |exp_l, exp_r| {
        let exp_kind =
            ExpKind::Cmp(CmpOp::Bool(BoolCmpOp::Eq), OpTyp::Bool, Box::new(exp_l), Box::new(exp_r));
        crate::note_phrase!(node: exp_kind, note: TypKind::Bool, span: span(1))
    };
    let output = |text| instr(InstrKind::Return(ReturnInstr { exp: literal(text) }));
    let branch = |exp, text| {
        let block = vec![output(text)];
        instr(InstrKind::If(IfInstr { exp, iter_exps: vec![], block }))
    };
    let block = vec![
        branch(equality(exp_target.clone(), literal("a")), "A"),
        branch(equality(exp_target.clone(), literal("b")), "B"),
        branch(equality(literal("b"), exp_target.clone()), "C"),
    ];
    for without_rule_groups in [false, true] {
        let block = optimize(&TDEnv::new(), block.clone(), without_rule_groups).unwrap();
        assert_eq!(block.len(), 1);
        let InstrKind::Case(instr_case) = &block[0].node else { panic!("expected case") };
        assert_eq!(instr_case.exp, exp_target);
        assert_eq!(instr_case.cases.len(), 2);
        assert_eq!(
            instr_case.cases[0].guard,
            Guard::Cmp(CmpOp::Bool(BoolCmpOp::Eq), OpTyp::Bool, literal("a"))
        );
        assert_eq!(instr_case.cases[0].block, vec![output("A")]);
        assert_eq!(
            instr_case.cases[1].guard,
            Guard::Cmp(CmpOp::Bool(BoolCmpOp::Eq), OpTyp::Bool, literal("b"))
        );
        assert_eq!(instr_case.cases[1].block, vec![output("B"), output("C")]);
    }
}
