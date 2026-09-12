use super::*;
use crate::pass::structure::opt::r#loop::merge_binding::apply;
#[test]
fn test_adjacent_bindings_rename_and_preserve_tail_and_span() {
    for make in [binding as fn(&str, Block) -> Instr, rule] {
        let mut instr_a = make("a", vec![ret("a")]);
        instr_a.span = span(8);
        let mut instr_expect = make("a", vec![ret("a"), ret("a"), ret("a")]);
        instr_expect.span = span(8);
        assert_eq!(
            apply(vec![
                instr_a,
                make("b", vec![ret("b")]),
                make("c", vec![ret("c")]),
                ret("tail")
            ])
            .unwrap(),
            vec![instr_expect, ret("tail")]
        );
    }
}
#[test]
fn test_capture_avoidance_in_target_body() {
    let mut instr_inner = binding("a", vec![ret("b"), ret("a")]);
    if let InstrKind::Let(instr_let) = &mut instr_inner.node {
        instr_let.exp_r = variable("other");
    }
    let block = apply(vec![binding("a", vec![]), binding("b", vec![instr_inner])]).unwrap();
    let InstrKind::Let(instr_outer) = &block[0].node else {
        panic!("expected let")
    };
    let InstrKind::Let(instr_inner) = &instr_outer.block[0].node else {
        panic!("expected let")
    };
    let ExpKind::Var(id_fresh) = &instr_inner.exp_l.node else {
        panic!("expected variable")
    };
    assert_ne!(id_fresh.node, "a");
    assert_eq!(instr_inner.block[0], ret("a"));
    let InstrKind::Return(instr_return) = &instr_inner.block[1].node else {
        panic!("expected return")
    };
    let ExpKind::Var(id_return) = &instr_return.exp.node else {
        panic!("expected variable")
    };
    assert_eq!(id_return, id_fresh);
}
#[test]
fn test_iterator_filtering_binding_renaming_and_mismatch() {
    let mut instr_a = binding("a", vec![ret("a")]);
    let mut instr_b = binding("b", vec![ret("b")]);
    for (instr_ol, text) in [(&mut instr_a, "a"), (&mut instr_b, "b")] {
        let InstrKind::Let(instr_let) = &mut instr_ol.node else {
            unreachable!()
        };
        instr_let.iter_instrs = vec![InstrIter {
            iter: Iter::List,
            vars_bound: vec![var("input"), var(text)],
            vars_bind: vec![var(text), var("irrelevant")],
        }];
    }
    let mut instr_expect = instr_a.clone();
    if let InstrKind::Let(instr_let) = &mut instr_expect.node {
        instr_let.block.push(ret("a"));
    }
    assert_eq!(
        apply(vec![instr_a.clone(), instr_b.clone()]).unwrap(),
        vec![instr_expect]
    );
    if let InstrKind::Let(instr_let) = &mut instr_b.node {
        instr_let.iter_instrs[0].iter = Iter::Opt;
    }
    assert_eq!(
        apply(vec![instr_a.clone(), instr_b.clone()]).unwrap(),
        vec![instr_a, instr_b]
    );
}
#[test]
fn test_hint_split_compatibility_and_intervening_instruction() {
    let mut instr_a = rule("same", vec![ret("a")]);
    let mut instr_b = rule("same", vec![ret("b")]);
    for instr_ol in [&mut instr_a, &mut instr_b] {
        let InstrKind::Rule(instr_rule) = &mut instr_ol.node else {
            unreachable!()
        };
        instr_rule.not_exp = Mixfix::Seq(vec![
            Mixfix::Arg(variable("same")),
            Mixfix::Arg(variable("same")),
        ]);
    }
    if let InstrKind::Rule(instr_rule) = &mut instr_b.node {
        instr_rule.input_hint = InputHint::new(vec![1]);
    }
    let mut instr_expect = instr_a.clone();
    if let InstrKind::Rule(instr_rule) = &mut instr_expect.node {
        instr_rule.block.push(ret("b"));
    }
    assert_eq!(
        apply(vec![instr_a.clone(), instr_b.clone()]).unwrap(),
        vec![instr_expect]
    );
    if let InstrKind::Rule(instr_rule) = &mut instr_b.node {
        instr_rule.not_exp = Mixfix::Seq(vec![
            Mixfix::Arg(variable("same")),
            Mixfix::Arg(variable("different")),
        ]);
    }
    let block = vec![instr_a, instr_b];
    assert_eq!(apply(block.clone()).unwrap(), block);
    let block = vec![binding("a", vec![]), ret("barrier"), binding("b", vec![])];
    assert_eq!(apply(block.clone()).unwrap(), block);
}
#[test]
fn test_invalid_rule_hint_keeps_owning_span() {
    let mut instr_ol = rule("a", vec![]);
    instr_ol.span = span(17);
    if let InstrKind::Rule(instr_rule) = &mut instr_ol.node {
        instr_rule.input_hint = InputHint::new(vec![9]);
    }
    let error = apply(vec![instr_ol]).unwrap_err();
    assert_eq!(error.span, span(17));
    assert!(matches!(
        error.kind,
        crate::pass::structure::StructureErrorKind::Input(_)
    ));
}

fn pattern(exp_kind: ExpKind) -> Exp {
    crate::note_phrase! {node:exp_kind,note:TypKind::Bool,span:span(6)}
}
fn atom(text: &str) -> crate::lang::il::ast::Atom {
    crate::phrase! {node:crate::lang::common::notation::atom::Atom::Keyword(text.into()),span:span(5)}
}
fn patterns(text: &str) -> Vec<Exp> {
    vec![
        pattern(ExpKind::Tuple(vec![variable(text)])),
        pattern(ExpKind::Case(Box::new(Mixfix::Arg(variable(text))))),
        pattern(ExpKind::Str(vec![(atom("field"), variable(text))])),
        pattern(ExpKind::Opt(Some(Box::new(variable(text))))),
        pattern(ExpKind::Opt(None)),
        pattern(ExpKind::List(vec![variable(text)])),
        pattern(ExpKind::Cons(
            Box::new(variable(text)),
            Box::new(variable(text)),
        )),
        pattern(ExpKind::Iter(
            Box::new(variable(text)),
            (Iter::List, vec![var(text)]),
        )),
    ]
}
#[test]
fn test_structured_binding_patterns_and_shape_negatives() {
    for (exp_a, exp_b) in patterns("a").into_iter().zip(patterns("b")) {
        let mut instr_a = binding("a", vec![]);
        let text_return = if matches!(exp_a.node, ExpKind::Opt(None)) {
            "b"
        } else {
            "a"
        };
        let mut instr_b = binding("b", vec![ret("b")]);
        if let InstrKind::Let(instr_let) = &mut instr_a.node {
            instr_let.exp_l = exp_a;
        }
        if let InstrKind::Let(instr_let) = &mut instr_b.node {
            instr_let.exp_l = exp_b;
        }
        let mut instr_expect = instr_a.clone();
        if let InstrKind::Let(instr_let) = &mut instr_expect.node {
            instr_let.block = vec![ret(text_return)];
        }
        assert_eq!(apply(vec![instr_a, instr_b]).unwrap(), vec![instr_expect]);
    }
    for (exp_a, exp_b) in [
        (
            pattern(ExpKind::Str(vec![(atom("field_a"), variable("a"))])),
            pattern(ExpKind::Str(vec![(atom("field_b"), variable("b"))])),
        ),
        (pattern(ExpKind::Bool(true)), pattern(ExpKind::Bool(true))),
        (
            pattern(ExpKind::Tuple(vec![])),
            pattern(ExpKind::Tuple(vec![variable("b")])),
        ),
        (
            pattern(ExpKind::Opt(None)),
            pattern(ExpKind::Opt(Some(Box::new(variable("b"))))),
        ),
        (
            pattern(ExpKind::Iter(Box::new(variable("a")), (Iter::List, vec![]))),
            pattern(ExpKind::Iter(Box::new(variable("b")), (Iter::Opt, vec![]))),
        ),
        (
            pattern(ExpKind::Case(Box::new(Mixfix::Arg(variable("a"))))),
            pattern(ExpKind::Case(Box::new(Mixfix::Seq(vec![Mixfix::Arg(
                variable("b"),
            )])))),
        ),
    ] {
        let mut instr_a = binding("a", vec![]);
        let mut instr_b = binding("b", vec![]);
        if let InstrKind::Let(instr_let) = &mut instr_a.node {
            instr_let.exp_l = exp_a;
        }
        if let InstrKind::Let(instr_let) = &mut instr_b.node {
            instr_let.exp_l = exp_b;
        }
        let block = vec![instr_a, instr_b];
        assert_eq!(apply(block.clone()).unwrap(), block);
    }
}
#[test]
fn test_repeated_pattern_variables_follow_source_mapping_order() {
    let mut instr_a = binding("a", vec![]);
    let mut instr_b = binding("b", vec![ret("target")]);
    if let InstrKind::Let(instr_let) = &mut instr_a.node {
        instr_let.exp_l = pattern(ExpKind::Tuple(vec![variable("a"), variable("b")]));
    }
    if let InstrKind::Let(instr_let) = &mut instr_b.node {
        instr_let.exp_l = pattern(ExpKind::Tuple(vec![variable("target"), variable("target")]));
    }
    let mut instr_expect = instr_a.clone();
    if let InstrKind::Let(instr_let) = &mut instr_expect.node {
        instr_let.block = vec![ret("b")];
    }
    assert_eq!(apply(vec![instr_a, instr_b]).unwrap(), vec![instr_expect]);
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
    let block = vec![binding("a", vec![ret("a")]), binding("b", vec![ret("b")])];
    let block_expect = vec![binding("a", vec![ret("a"), ret("a")])];
    assert_eq!(apply(nested(block.clone())).unwrap(), nested(block_expect));
    let instr_debug = instr(InstrKind::Debug(DebugInstr {
        exp: variable("debug"),
        instr: Box::new(binding("outer", block)),
    }));
    assert_eq!(apply(vec![instr_debug.clone()]).unwrap(), vec![instr_debug]);
}
