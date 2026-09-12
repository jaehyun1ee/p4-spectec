use super::*;
use crate::pass::structure::opt::r#loop::merge_hold::apply;
#[test]
fn test_both_outcomes_merge_in_order_and_adopt_target_condition_metadata() {
    let mut instr_a = hold(vec![ret("a")], vec![ret("not_a")]);
    instr_a.span = span(10);
    let mut instr_b = hold(vec![ret("b")], vec![ret("not_b")]);
    if let InstrKind::Hold(instr_hold) = &mut instr_b.node {
        instr_hold.id.span = span(20);
    }
    let mut instr_expect = instr_b.clone();
    instr_expect.span = span(10);
    if let InstrKind::Hold(instr_hold) = &mut instr_expect.node {
        instr_hold.block_hold = vec![ret("a"), ret("b")];
        instr_hold.block_not_hold = vec![ret("not_a"), ret("not_b")];
    }
    assert_eq!(
        apply(vec![instr_a, instr_b, ret("tail")]),
        vec![instr_expect, ret("tail")]
    );
}
#[test]
fn test_three_holds_and_both_one_sided_outcomes() {
    assert_eq!(
        apply(vec![
            hold(vec![ret("a")], vec![]),
            hold(vec![], vec![ret("b")]),
            hold(vec![ret("c")], vec![])
        ]),
        vec![hold(vec![ret("a"), ret("c")], vec![ret("b")])]
    );
}
#[test]
fn test_condition_iterator_and_barrier_mismatch() {
    let instr_a = hold(vec![ret("a")], vec![]);
    for idx in 0..3 {
        let mut instr_b = hold(vec![ret("b")], vec![]);
        let InstrKind::Hold(instr_hold) = &mut instr_b.node else {
            unreachable!()
        };
        match idx {
            0 => instr_hold.id = id("other"),
            1 => instr_hold.not_exp = Mixfix::Arg(variable("other")),
            _ => instr_hold.iter_exps = vec![(Iter::List, vec![])],
        };
        let block = vec![instr_a.clone(), instr_b];
        assert_eq!(apply(block.clone()), block);
    }
    let block = vec![instr_a.clone(), ret("barrier"), instr_a];
    assert_eq!(apply(block.clone()), block);
}

fn nested(block: Block) -> Block {
    let block = vec![rule("output", block)];
    let block = vec![binding("bound", block)];
    let block = vec![instr(InstrKind::Group(GroupInstr {
        id: id("group"),
        rel_signature: super::super::super::signature(),
        exps: vec![variable("group_input")],
        block,
    }))];
    let block = vec![instr(InstrKind::Case(CaseInstr {
        exp: variable("case"),
        cases: vec![Case {
            guard: Guard::Bool(true),
            block,
        }],
        total: false,
    }))];
    let block = vec![hold(block.clone(), block)];
    vec![instr(InstrKind::If(IfInstr {
        exp: variable("condition"),
        iter_exps: vec![(Iter::List, vec![])],
        block,
    }))]
}

#[test]
fn test_nested_blocks_and_debug_barrier() {
    let block = vec![hold(vec![ret("a")], vec![]), hold(vec![], vec![ret("b")])];
    let block_expect = vec![hold(vec![ret("a")], vec![ret("b")])];
    assert_eq!(apply(nested(block.clone())), nested(block_expect));
    let instr_debug = instr(InstrKind::Debug(DebugInstr {
        exp: variable("debug"),
        instr: Box::new(binding("outer", block)),
    }));
    assert_eq!(apply(vec![instr_debug.clone()]), vec![instr_debug]);
}

#[test]
fn test_each_outcome_merges_common_leading_conditions() {
    fn branch(text: &str) -> Block {
        vec![instr(InstrKind::If(IfInstr {
            exp: variable("condition"),
            iter_exps: vec![],
            block: vec![ret(text)],
        }))]
    }
    fn branch_merged(text_a: &str, text_b: &str) -> Block {
        vec![instr(InstrKind::If(IfInstr {
            exp: variable("condition"),
            iter_exps: vec![],
            block: vec![ret(text_a), ret(text_b)],
        }))]
    }
    assert_eq!(
        apply(vec![
            hold(branch("a"), branch("not_a")),
            hold(branch("b"), branch("not_b"))
        ]),
        vec![hold(
            branch_merged("a", "b"),
            branch_merged("not_a", "not_b")
        )]
    );
}
