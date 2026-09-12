use super::*;
use crate::pass::structure::opt::pre::remove_let_alias::apply;
#[test]
fn test_alias_chain_and_shadowing() {
    let exp_literal =
        crate::note_phrase! {node: ExpKind::Bool(true), note: TypKind::Bool, span: span(3)};
    let instr_shadow = binding(variable("y"), exp_literal, vec![ret("y")]);
    let block = apply(vec![
        binding(
            variable("y"),
            variable("x"),
            vec![binding(
                variable("z"),
                variable("y"),
                vec![ret("z"), instr_shadow.clone()],
            )],
        ),
        ret("y"),
    ])
    .unwrap();
    assert_eq!(block, vec![ret("x"), instr_shadow, ret("y")]);
}
#[test]
fn test_iterated_aliases_and_replacement_capture() {
    for iter in [Iter::List, Iter::Opt] {
        let block = apply(vec![binding(
            iterated("y", iter),
            iterated("x", iter),
            vec![ret("y")],
        )])
        .unwrap();
        assert_eq!(block, vec![ret("x")]);
    }
    let exp_target = iterated("x", Iter::List);
    let exp_literal =
        crate::note_phrase! {node: ExpKind::Bool(true), note: TypKind::Bool, span: span(3)};
    let block = apply(vec![binding(
        variable("y"),
        exp_target.clone(),
        vec![binding(
            variable("x"),
            exp_literal,
            vec![ret("y"), ret("x")],
        )],
    )])
    .unwrap();
    let InstrKind::Let(instr_let) = &block[0].node else {
        panic!("expected let")
    };
    let ExpKind::Var(id_fresh) = &instr_let.exp_l.node else {
        panic!("expected variable")
    };
    assert_ne!(id_fresh.node, "x");
    let InstrKind::Return(instr_return) = &instr_let.block[0].node else {
        panic!("expected return")
    };
    assert_eq!(instr_return.exp, exp_target);
    let InstrKind::Return(instr_return) = &instr_let.block[1].node else {
        panic!("expected return")
    };
    let ExpKind::Var(id_return) = &instr_return.exp.node else {
        panic!("expected variable")
    };
    assert_eq!(id_return, id_fresh);
}
#[test]
fn test_unequal_iterators_and_debug_barrier() {
    let instr_let = binding(
        iterated("y", Iter::Opt),
        iterated("x", Iter::List),
        vec![ret("y")],
    );
    let instr_debug = instr(InstrKind::Debug(DebugInstr {
        exp: variable("y"),
        instr: Box::new(binding(variable("y"), variable("x"), vec![ret("y")])),
    }));
    assert_eq!(
        apply(vec![instr_let.clone(), instr_debug.clone()]).unwrap(),
        vec![instr_let, instr_debug.clone()]
    );
    let block = apply(vec![binding(
        variable("y"),
        variable("z"),
        vec![instr_debug],
    )])
    .unwrap();
    let InstrKind::Debug(instr_debug) = &block[0].node else {
        panic!("expected debug")
    };
    assert_eq!(instr_debug.exp, variable("z"));
    assert!(matches!(instr_debug.instr.node, InstrKind::Let(_)));
}

#[test]
fn test_all_nested_blocks_preserve_annotations() {
    let block = vec![binding(variable("y"), variable("x"), vec![ret("y")])];
    assert_eq!(
        apply(vec![group(containers(block))]).unwrap(),
        vec![group(containers(vec![ret("x")]))]
    );
}

#[test]
fn test_alias_substitution_propagates_located_rule_errors() {
    use crate::lang::{
        common::notation::mixfix::Mixfix,
        hints::input::{InputError, InputHint},
    };
    use crate::pass::structure::StructureErrorKind;
    for exp_r in [variable("x"), iterated("x", Iter::List)] {
        let mut instr_rule = instr(InstrKind::Rule(RuleInstr {
            id: id("relation"),
            not_exp: Mixfix::Arg(variable("y")),
            input_hint: InputHint::new(vec![2]),
            iter_instrs: vec![],
            block: vec![],
        }));
        instr_rule.span = span(9);
        let error = apply(vec![binding(variable("y"), exp_r, vec![instr_rule])]).unwrap_err();
        assert_eq!(error.span, span(9));
        assert_eq!(
            error.kind,
            StructureErrorKind::Input(InputError::IndexOutOfBounds { index: 2, arity: 1 })
        );
    }
}
