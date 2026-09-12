use super::*;
use crate::pass::structure::{StructureErrorKind, opt::post::remove_let_dead::apply};
#[test]
fn test_dead_pure_bindings_disappear_but_live_and_calls_remain() {
    assert_eq!(
        apply(vec![binding(literal(), vec![ret("other")])]).unwrap(),
        vec![ret("other")]
    );
    let instr_live = binding(literal(), vec![ret("x")]);
    let exp_call = crate::note_phrase!(node: ExpKind::Call(id("effect"), vec![], vec![]), note: TypKind::Bool, span: span(8));
    let instr_call = binding(
        crate::note_phrase!(node: ExpKind::Tuple(vec![exp_call]), note: TypKind::Tuple(vec![]), span: span(9)),
        vec![ret("other")],
    );
    assert_eq!(
        apply(vec![instr_live.clone(), instr_call.clone()]).unwrap(),
        vec![instr_live, instr_call]
    );
}
#[test]
fn test_downstream_shadowing_and_rule_input_output_roles() {
    let instr_rule = rule(vec![], InputHint::new(vec![0]));
    assert_eq!(
        apply(vec![binding(literal(), vec![instr_rule.clone(), ret("x")])]).unwrap(),
        vec![instr_rule.clone(), ret("x")]
    );
    let instr_input = rule(vec![], InputHint::new(vec![1]));
    let instr_live = binding(literal(), vec![instr_input]);
    assert_eq!(apply(vec![instr_live.clone()]).unwrap(), vec![instr_live]);
    let instr_nested = binding(literal(), vec![ret("other")]);
    assert_eq!(
        apply(vec![binding(literal(), vec![instr_nested, ret("x")])]).unwrap(),
        vec![ret("other"), ret("x")]
    );
    let instr_live = binding(
        literal(),
        vec![rule(vec![ret("x")], InputHint::new(vec![0]))],
    );
    assert_eq!(apply(vec![instr_live.clone()]).unwrap(), vec![instr_live]);
}
#[test]
fn test_debug_contributes_liveness_without_rewriting_its_instruction() {
    let instr_debug = instr(InstrKind::Debug(DebugInstr {
        exp: variable("x"),
        instr: Box::new(binding(literal(), vec![ret("other")])),
    }));
    let instr_live = binding(literal(), vec![instr_debug]);
    assert_eq!(apply(vec![instr_live.clone()]).unwrap(), vec![instr_live]);
}
#[test]
fn test_invalid_downstream_rule_hint_retains_rule_span() {
    let mut instr_rule = rule(vec![], InputHint::new(vec![3]));
    instr_rule.span = span(12);
    let error = apply(vec![binding(literal(), vec![instr_rule])]).unwrap_err();
    assert_eq!(error.span, span(12));
    assert_eq!(
        error.kind,
        StructureErrorKind::Input(crate::lang::hints::input::InputError::IndexOutOfBounds {
            index: 3,
            arity: 2
        })
    );
}

#[test]
fn test_liveness_uses_each_container_expression_and_branch() {
    let blocks = vec![
        vec![instr(InstrKind::If(IfInstr {
            exp: variable("x"),
            iter_exps: vec![],
            block: vec![],
        }))],
        vec![instr(InstrKind::Hold(HoldInstr {
            id: id("R"),
            not_exp: Mixfix::Arg(variable("other")),
            iter_exps: vec![],
            block_hold: vec![],
            block_not_hold: vec![ret("x")],
        }))],
        vec![instr(InstrKind::Case(CaseInstr {
            exp: variable("other"),
            cases: vec![Case {
                guard: Guard::Mem(variable("x")),
                block: vec![],
            }],
            total: false,
        }))],
        vec![instr(InstrKind::Group(GroupInstr {
            id: id("G"),
            rel_signature: signature(),
            exps: vec![variable("x")],
            block: vec![],
        }))],
        vec![instr(InstrKind::Result(ResultInstr {
            rel_signature: signature(),
            exps: vec![variable("x")],
        }))],
    ];
    for block in blocks {
        let instr_live = binding(literal(), block);
        assert_eq!(apply(vec![instr_live.clone()]).unwrap(), vec![instr_live]);
    }
}
#[test]
fn test_downstream_uses_are_computed_before_recursive_deletion() {
    let exp_tuple = crate::note_phrase!(node: ExpKind::Tuple(vec![variable("x")]), note: TypKind::Tuple(vec![]), span: span(8));
    let instr_inner = instr(InstrKind::Let(LetInstr {
        exp_l: variable("y"),
        exp_r: exp_tuple,
        iter_instrs: vec![],
        block: vec![],
    }));
    assert_eq!(
        apply(vec![binding(literal(), vec![instr_inner])]).unwrap(),
        vec![binding(literal(), vec![])]
    );
}
#[test]
fn test_update_path_is_not_part_of_source_removability() {
    use crate::lang::il::ast::PathKind;
    let exp_call = crate::note_phrase!(node: ExpKind::Call(id("index"), vec![], vec![]), note: TypKind::Num(crate::lang::xl::num::Typ::Nat), span: span(8));
    let path_root = crate::note_phrase!(node: PathKind::Root, note: TypKind::Bool, span: span(9));
    let path = crate::note_phrase!(node: PathKind::Idx(Box::new(path_root), Box::new(exp_call)), note: TypKind::Bool, span: span(10));
    let exp_update = crate::note_phrase!(node: ExpKind::Upd(Box::new(variable("base")), Box::new(path), Box::new(literal())), note: TypKind::Bool, span: span(11));
    assert_eq!(
        apply(vec![binding(exp_update, vec![ret("other")])]).unwrap(),
        vec![ret("other")]
    );
}
