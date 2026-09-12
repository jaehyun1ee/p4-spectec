use crate::{
    lang::{
        common::source::{Position, Span},
        il::ast::{Exp, ExpKind, TypKind},
    },
    pass::structure::{ol::ast::*, opt::r#loop::merge_if},
    runtime::envs::algo::TDEnv,
};
fn var(text: &str) -> Exp {
    crate::note_phrase!(node: ExpKind::Var(crate::phrase!(node: text.into(), span: Default::default())), note: TypKind::Bool, span: Default::default())
}
fn span(int_line: i64) -> Span {
    Span::new(
        Position::new("conditions", int_line, 1),
        Position::new("conditions", int_line, 9),
    )
}
fn ret(text: &str) -> Instr {
    crate::phrase!(node: InstrKind::Return(ReturnInstr { exp: var(text) }), span: Default::default())
}
fn branch(exp: Exp, text: &str, int_line: i64) -> Instr {
    crate::phrase!(node: InstrKind::If(IfInstr { exp, iter_exps: vec![], block: vec![ret(text)] }), span: span(int_line))
}
#[test]
fn test_identical_search_preserves_intervening_if_tail_and_target_span() {
    let instr_a = branch(var("p"), "a", 1);
    let instr_b = branch(var("q"), "b", 2);
    let instr_c = branch(var("p"), "c", 3);
    let instr_tail = ret("tail");
    let block = merge_if::apply(
        &TDEnv::new(),
        vec![instr_a, instr_b.clone(), instr_c, instr_tail.clone()],
    )
    .unwrap();
    assert_eq!(block.len(), 3);
    assert_eq!(block[0].span, span(1));
    assert_eq!(block[1], instr_b);
    assert_eq!(block[2], instr_tail);
    let InstrKind::If(instr_if) = &block[0].node else {
        panic!("expected if")
    };
    assert_eq!(instr_if.block, vec![ret("a"), ret("c")]);
}

#[test]
fn test_barrier_fuzzy_and_downstream_blocks() {
    let block_input = vec![
        branch(var("p"), "a", 1),
        ret("barrier"),
        branch(var("p"), "b", 2),
    ];
    assert_eq!(
        merge_if::apply(&TDEnv::new(), block_input.clone()).unwrap(),
        block_input
    );
    let block_input = vec![branch(var("p"), "a", 1), branch(var("q"), "b", 2)];
    assert_eq!(
        merge_if::apply(&TDEnv::new(), block_input.clone()).unwrap(),
        block_input
    );
    let block_inner = vec![branch(var("p"), "a", 2), branch(var("p"), "b", 3)];
    let instr_outer = crate::phrase!(node: InstrKind::If(IfInstr { exp: var("outer"), iter_exps: vec![], block: block_inner.clone() }),span: span(1));
    let instr_debug = crate::phrase!(node: InstrKind::Debug(DebugInstr { exp: var("debug"), instr: Box::new(instr_outer.clone()) }),span: span(9));
    let block = merge_if::apply(&TDEnv::new(), vec![instr_outer, instr_debug.clone()]).unwrap();
    let InstrKind::If(instr_if) = &block[0].node else {
        panic!("expected if")
    };
    assert_eq!(instr_if.block.len(), 1);
    assert_eq!(block[1], instr_debug);
}

fn nested(block: Block) -> Block {
    let block = vec![super::rule("output", block)];
    let block = vec![super::binding("bound", block)];
    let block = vec![
        crate::phrase!(node: InstrKind::Group(GroupInstr {id: super::id("group"), rel_signature: super::super::super::signature(), exps: vec![var("input")],block}),span: span(7)),
    ];
    let block = vec![
        crate::phrase!(node: InstrKind::Case(CaseInstr {exp: var("outer_case"),cases: vec![Case {guard: Guard::Bool(true),block}],total: true}),span: span(8)),
    ];
    vec![super::hold(block.clone(), block)]
}
#[test]
fn test_recursive_hold_case_group_let_rule_bodies() {
    let block = vec![branch(var("p"), "a", 1), branch(var("p"), "b", 2)];
    let block_expect = merge_if::apply(&TDEnv::new(), block.clone()).unwrap();
    assert_eq!(
        merge_if::apply(&TDEnv::new(), nested(block.clone())).unwrap(),
        nested(block_expect)
    );
    let instr_debug = crate::phrase!(node: InstrKind::Debug(DebugInstr {exp: var("debug"),instr: Box::new(super::binding("bound",block))}),span: span(9));
    assert_eq!(
        merge_if::apply(&TDEnv::new(), vec![instr_debug.clone()]).unwrap(),
        vec![instr_debug]
    );
}

#[test]
fn test_iterator_compatibility_and_partition_conditions_remain_separate() {
    use crate::lang::il::ast::{Iter, OpTyp, UnOp};
    let mut instr_a = branch(var("p"), "a", 1);
    let mut instr_b = branch(var("p"), "b", 2);
    if let InstrKind::If(instr_if) = &mut instr_a.node {
        instr_if.iter_exps = vec![(Iter::List, vec![])];
    }
    let block_input = vec![instr_a.clone(), instr_b.clone()];
    assert_eq!(
        merge_if::apply(&TDEnv::new(), block_input.clone()).unwrap(),
        block_input
    );
    if let InstrKind::If(instr_if) = &mut instr_b.node {
        instr_if.iter_exps = vec![(Iter::List, vec![])];
    }
    let block = merge_if::apply(&TDEnv::new(), vec![instr_a, instr_b]).unwrap();
    assert_eq!(block.len(), 1);
    let exp_not = crate::note_phrase!(node: ExpKind::Un(UnOp::Bool(crate::lang::xl::bool::UnOp::Not),OpTyp::Bool,Box::new(var("p"))),note: TypKind::Bool,span: Default::default());
    let block_input = vec![branch(var("p"), "a", 1), branch(exp_not, "b", 2)];
    assert_eq!(
        merge_if::apply(&TDEnv::new(), block_input.clone()).unwrap(),
        block_input
    );
}
