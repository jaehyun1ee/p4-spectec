use crate::{
    lang::{
        common::source::{Position, Span},
        il::ast::{Exp, ExpKind, TypKind},
    },
    pass::structure::{ol::ast::*, opt::r#loop::casify},
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
fn neg(exp: Exp) -> Exp {
    crate::note_phrase!(node: ExpKind::Un(crate::lang::il::ast::UnOp::Bool(crate::lang::xl::bool::UnOp::Not), crate::lang::il::ast::OpTyp::Bool, Box::new(exp)), note: TypKind::Bool, span: Default::default())
}
#[test]
fn test_partition_becomes_total_case_and_preserves_tail_span() {
    let instr_tail = ret("tail");
    let block = casify::apply(
        &TDEnv::new(),
        vec![
            branch(var("p"), "a", 1),
            branch(neg(var("p")), "b", 2),
            instr_tail.clone(),
        ],
    )
    .unwrap();
    assert_eq!(block.len(), 2);
    assert_eq!(block[0].span, span(1));
    assert_eq!(block[1], instr_tail);
    let InstrKind::Case(instr_case) = &block[0].node else {
        panic!("expected case")
    };
    assert!(instr_case.total);
    assert_eq!(instr_case.cases[0].guard, Guard::Bool(true));
    assert_eq!(instr_case.cases[1].guard, Guard::Bool(false));
}

fn cmp(text: &str) -> Exp {
    crate::note_phrase!(node: ExpKind::Cmp(crate::lang::il::ast::CmpOp::Bool(crate::lang::xl::bool::CmpOp::Eq), crate::lang::il::ast::OpTyp::Bool, Box::new(var("p")), Box::new(crate::note_phrase!(node: ExpKind::Text(text.into()), note: TypKind::Text, span: Default::default()))), note: TypKind::Bool, span: Default::default())
}
fn guard(text: &str) -> Guard {
    crate::pass::structure::opt::overlap::exp_as_guard(&var("p"), &cmp(text)).unwrap()
}
fn case(cases: &[(&str, &str)], total: bool, int_line: i64) -> Instr {
    crate::phrase!(node: InstrKind::Case(CaseInstr { exp: var("p"), cases: cases.iter().map(|(text_guard,text_block)| Case { guard: guard(text_guard), block: vec![ret(text_block)] }).collect(), total }), span: span(int_line))
}
#[test]
fn test_disjoint_partial_fuzzy_and_if_search_order() {
    let instr_fuzzy = branch(var("q"), "fuzzy", 2);
    let instr_tail = ret("tail");
    let block = casify::apply(
        &TDEnv::new(),
        vec![
            branch(cmp("a"), "a", 1),
            instr_fuzzy.clone(),
            branch(cmp("b"), "b", 3),
            instr_tail.clone(),
        ],
    )
    .unwrap();
    assert_eq!(block.len(), 3);
    assert_eq!(block[1], instr_fuzzy);
    assert_eq!(block[2], instr_tail);
    let InstrKind::Case(instr_case) = &block[0].node else {
        panic!("expected case")
    };
    assert!(!instr_case.total);
    assert_eq!(
        instr_case
            .cases
            .iter()
            .map(|case| case.guard.clone())
            .collect::<Vec<_>>(),
        vec![guard("a"), guard("b")]
    );
    let block_input = vec![
        branch(cmp("a"), "a", 1),
        ret("barrier"),
        branch(cmp("b"), "b", 2),
    ];
    assert_eq!(
        casify::apply(&TDEnv::new(), block_input.clone()).unwrap(),
        block_input
    );
}
#[test]
fn test_if_case_and_case_if_preserve_source_identical_priority_and_flags() {
    for total in [false, true] {
        let block = casify::apply(
            &TDEnv::new(),
            vec![
                branch(cmp("b"), "target", 1),
                case(&[("a", "skip"), ("b", "match"), ("c", "tail")], total, 2),
            ],
        )
        .unwrap();
        assert_eq!(block[0].span, span(1));
        let InstrKind::Case(instr_case) = &block[0].node else {
            panic!("expected case")
        };
        assert_eq!(instr_case.total, total);
        assert_eq!(instr_case.cases.len(), 2);
        assert_eq!(instr_case.cases[0].guard, guard("b"));
        assert_eq!(instr_case.cases[0].block, vec![ret("target"), ret("match")]);
        assert_eq!(instr_case.cases[1].block, vec![ret("tail")]);
        let block = casify::apply(
            &TDEnv::new(),
            vec![
                case(&[("a", "skip"), ("b", "match"), ("c", "tail")], total, 1),
                branch(cmp("b"), "target", 2),
            ],
        )
        .unwrap();
        let InstrKind::Case(instr_case) = &block[0].node else {
            panic!("expected case")
        };
        assert!(!instr_case.total);
        assert_eq!(instr_case.cases.len(), 2);
        assert_eq!(instr_case.cases[0].block, vec![ret("match"), ret("target")]);
    }
}
#[test]
fn test_partial_case_append_and_case_case_total_target() {
    for block_input in [
        vec![branch(cmp("b"), "b", 1), case(&[("a", "a")], false, 2)],
        vec![case(&[("a", "a")], false, 1), branch(cmp("b"), "b", 2)],
    ] {
        let block = casify::apply(&TDEnv::new(), block_input).unwrap();
        let InstrKind::Case(instr_case) = &block[0].node else {
            panic!("expected case")
        };
        assert!(!instr_case.total);
        assert_eq!(
            instr_case
                .cases
                .iter()
                .map(|case| case.guard.clone())
                .collect::<Vec<_>>(),
            vec![guard("a"), guard("b")]
        );
    }
    for total in [false, true] {
        let instr_tail = ret("tail");
        let block = casify::apply(
            &TDEnv::new(),
            vec![
                case(&[("a", "a"), ("b", "b")], total, 1),
                case(&[("a", "c")], !total, 2),
                instr_tail.clone(),
            ],
        )
        .unwrap();
        assert_eq!(block[1], instr_tail);
        assert_eq!(block[0].span, span(1));
        let InstrKind::Case(instr_case) = &block[0].node else {
            panic!("expected case")
        };
        assert_eq!(instr_case.total, total);
        assert_eq!(instr_case.cases[0].block, vec![ret("a"), ret("c")]);
        assert_eq!(instr_case.cases[1].block, vec![ret("b")]);
    }
}
#[test]
fn test_total_case_exhaustion_is_located_at_owning_case() {
    for (block_input, span_expect) in [
        (vec![branch(cmp("b"), "b", 1), case(&[], true, 2)], span(2)),
        (
            vec![case(&[("a", "a")], true, 3), branch(cmp("b"), "b", 4)],
            span(3),
        ),
        (
            vec![case(&[], true, 5), case(&[("a", "a")], false, 6)],
            span(5),
        ),
    ] {
        let error = casify::apply(&TDEnv::new(), block_input).unwrap_err();
        assert_eq!(
            error.kind,
            crate::pass::structure::error::StructureErrorKind::EmptyTotalCase
        );
        assert_eq!(error.span, span_expect);
    }
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
    let block = vec![branch(var("p"), "a", 1), branch(neg(var("p")), "b", 2)];
    let block_expect = casify::apply(&TDEnv::new(), block.clone()).unwrap();
    assert_eq!(
        casify::apply(&TDEnv::new(), nested(block.clone())).unwrap(),
        nested(block_expect)
    );
    let instr_debug = crate::phrase!(node: InstrKind::Debug(DebugInstr {exp: var("debug"),instr: Box::new(super::binding("bound",block))}),span: span(9));
    assert_eq!(
        casify::apply(&TDEnv::new(), vec![instr_debug.clone()]).unwrap(),
        vec![instr_debug]
    );
}

#[test]
fn test_iteration_blocks_search_and_different_case_targets_stay_separate() {
    let mut instr_iter = branch(neg(var("p")), "iter", 2);
    if let InstrKind::If(instr_if) = &mut instr_iter.node {
        instr_if.iter_exps = vec![(crate::lang::il::ast::Iter::List, vec![])];
    }
    for block_input in [
        vec![
            branch(var("p"), "a", 1),
            instr_iter.clone(),
            branch(neg(var("p")), "b", 3),
        ],
        vec![instr_iter, branch(var("p"), "b", 3)],
    ] {
        assert_eq!(
            casify::apply(&TDEnv::new(), block_input.clone()).unwrap(),
            block_input
        );
    }
    let mut instr_b = case(&[("b", "b")], false, 2);
    if let InstrKind::Case(instr_case) = &mut instr_b.node {
        instr_case.exp = var("q");
    }
    let block_input = vec![case(&[("a", "a")], false, 1), instr_b];
    assert_eq!(
        casify::apply(&TDEnv::new(), block_input.clone()).unwrap(),
        block_input
    );
}
#[test]
fn test_identical_subtype_guard_retains_case_proof_and_expression_span() {
    use crate::lang::il::ast::Subcheck;
    use crate::pass::structure::opt::overlap::guard_as_exp;
    let mut exp_target = var("p");
    exp_target.span = span(11);
    let typ = crate::phrase!(node: TypKind::Bool,span: span(12));
    let guard_case = Guard::Sub(typ.clone(), Box::new(Subcheck::Tuple(vec![Subcheck::Skip])));
    let guard_if = Guard::Sub(typ, Box::new(Subcheck::Skip));
    let instr_if = branch(guard_as_exp(&exp_target, &guard_if), "a", 1);
    let instr_case = crate::phrase!(node: InstrKind::Case(CaseInstr {exp: exp_target.clone(), cases: vec![Case {guard: guard_case.clone(),block: vec![ret("b")]}],total: true}),span: span(2));
    let block = casify::apply(&TDEnv::new(), vec![instr_if, instr_case]).unwrap();
    let InstrKind::Case(instr_case) = &block[0].node else {
        panic!("expected case")
    };
    assert_eq!(instr_case.exp, exp_target);
    assert_eq!(instr_case.cases[0].guard, guard_case);
    assert_eq!(instr_case.cases[0].block, vec![ret("a"), ret("b")]);
}
