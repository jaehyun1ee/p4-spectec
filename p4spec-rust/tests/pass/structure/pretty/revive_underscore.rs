use super::super::{ret, variable};
use crate::lang::traits::free::Free;
use crate::pass::structure::pretty::{RelBody, revive_underscore};
#[test]
fn test_revival_avoids_free_capture_and_agrees_with_fallback() {
    let body = revive_underscore::apply_rel(RelBody {
        exps_match: vec![variable("_x")],
        block: vec![ret("_x"), ret("x")],
        block_else: Some(vec![ret("_x")]),
    })
    .unwrap();
    let id = body.exps_match[0].free().iter().next().unwrap().clone();
    assert_ne!(id.node, "_x");
    assert_ne!(id.node, "x");
    assert!(body.block[0].free().contains(&id));
    assert!(body.block_else.unwrap()[0].free().contains(&id));
}

use super::super::{id, instr, signature, span};
use super::{binding, iterator, return_exp, var_id};
use crate::lang::{common::notation::mixfix::Mixfix, hints::input::InputHint};
use crate::pass::structure::ol::ast::*;
#[test]
fn test_unused_inputs_stay_underscored_and_let_scope_shadows_input() {
    let body = revive_underscore::apply_rel(RelBody {
        exps_match: vec![variable("_unused"), variable("_x")],
        block: vec![binding("_x", "_x", vec![ret("_x")])],
        block_else: None,
    })
    .unwrap();
    assert_eq!(var_id(&body.exps_match[0]).node, "_unused");
    let id_input = var_id(&body.exps_match[1]);
    let InstrKind::Let(instr_let) = &body.block[0].node else {
        panic!("expected let")
    };
    assert_eq!(var_id(&instr_let.exp_r), id_input);
    assert_ne!(var_id(&instr_let.exp_l).node, id_input.node);
    assert_eq!(
        var_id(return_exp(&instr_let.block[0])),
        var_id(&instr_let.exp_l)
    );
}
#[test]
fn test_rule_iterator_scopes_and_used_guard_survive_nested_branches() {
    let instr_rule = instr(InstrKind::Rule(RuleInstr {
        id: id("rel"),
        not_exp: Mixfix::Seq(vec![
            Mixfix::Arg(variable("_input")),
            Mixfix::Arg(variable("_out")),
        ]),
        input_hint: InputHint::new(vec![0]),
        iter_instrs: vec![iterator("_input", "_out")],
        block: vec![ret("_out"), ret("_input")],
    }));
    let instr_case = instr(InstrKind::Case(CaseInstr {
        exp: variable("_input"),
        total: true,
        cases: vec![Case {
            guard: Guard::Mem(variable("_input")),
            block: vec![instr_rule],
        }],
    }));
    let instr_hold = instr(InstrKind::Hold(HoldInstr {
        id: id("rel"),
        not_exp: Mixfix::Arg(variable("_input")),
        iter_exps: vec![],
        block_hold: vec![instr_case],
        block_not_hold: vec![ret("_input")],
    }));
    let instr_group = instr(InstrKind::Group(GroupInstr {
        id: id("group"),
        rel_signature: signature(),
        exps: vec![variable("_input")],
        block: vec![instr_hold],
    }));
    let body = revive_underscore::apply_rel(RelBody {
        exps_match: vec![variable("_input")],
        block: vec![instr_group],
        block_else: None,
    })
    .unwrap();
    let id_input = var_id(&body.exps_match[0]);
    let InstrKind::Group(instr_group) = &body.block[0].node else {
        panic!("expected group")
    };
    assert_eq!(var_id(&instr_group.exps[0]), id_input);
    let InstrKind::Hold(instr_hold) = &instr_group.block[0].node else {
        panic!("expected hold")
    };
    assert_eq!(var_id(return_exp(&instr_hold.block_not_hold[0])), id_input);
    let InstrKind::Case(instr_case) = &instr_hold.block_hold[0].node else {
        panic!("expected case")
    };
    let Guard::Mem(exp_guard) = &instr_case.cases[0].guard else {
        panic!("expected guard")
    };
    assert_eq!(var_id(exp_guard), id_input);
    let InstrKind::Rule(instr_rule) = &instr_case.cases[0].block[0].node else {
        panic!("expected rule")
    };
    let exps = instr_rule.not_exp.args();
    let id_output = var_id(exps[1]);
    assert_eq!(var_id(exps[0]), id_input);
    assert_eq!(&instr_rule.iter_instrs[0].vars_bound[0].id, id_input);
    assert_eq!(&instr_rule.iter_instrs[0].vars_bind[0].id, id_output);
    assert_eq!(var_id(return_exp(&instr_rule.block[0])), id_output);
    assert_ne!(id_output.node, "_out");
    assert_eq!(body.block[0].span, span(1));
}
#[test]
fn test_input_hint_failure_keeps_rule_span() {
    let mut instr_rule = instr(InstrKind::Rule(RuleInstr {
        id: id("rel"),
        not_exp: Mixfix::Arg(variable("_x")),
        input_hint: InputHint::new(vec![2]),
        iter_instrs: vec![],
        block: vec![],
    }));
    instr_rule.span = span(17);
    let error = revive_underscore::apply_rel(RelBody {
        exps_match: vec![],
        block: vec![instr_rule],
        block_else: None,
    })
    .unwrap_err();
    assert_eq!(error.span, span(17));
    assert!(matches!(
        error.kind,
        crate::pass::structure::StructureErrorKind::Input(_)
    ));
}

#[test]
fn test_let_iterator_bound_and_binding_names_follow_separate_scopes() {
    let instr_let = instr(InstrKind::Let(LetInstr {
        exp_l: variable("_local"),
        exp_r: variable("_input"),
        iter_instrs: vec![iterator("_input", "_local")],
        block: vec![
            ret("_local"),
            binding("_nested", "_local", vec![ret("_nested")]),
        ],
    }));
    let body = revive_underscore::apply_rel(RelBody {
        exps_match: vec![variable("_input")],
        block: vec![instr_let],
        block_else: None,
    })
    .unwrap();
    let id_input = var_id(&body.exps_match[0]);
    let InstrKind::Let(instr_let) = &body.block[0].node else {
        panic!("expected let")
    };
    let id_local = var_id(&instr_let.exp_l);
    assert_eq!(&instr_let.iter_instrs[0].vars_bound[0].id, id_input);
    assert_eq!(&instr_let.iter_instrs[0].vars_bind[0].id, id_local);
    assert_eq!(var_id(return_exp(&instr_let.block[0])), id_local);
    let InstrKind::Let(instr_nested) = &instr_let.block[1].node else {
        panic!("expected nested let")
    };
    assert_eq!(var_id(&instr_nested.exp_r), id_local);
    // Upstream revival stops at a binding and scans its body downstream
    assert_eq!(var_id(&instr_nested.exp_l).node, "_nested");
    assert_eq!(var_id(return_exp(&instr_nested.block[0])).node, "_nested");
}
