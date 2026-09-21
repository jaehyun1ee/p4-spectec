use super::super::{id_exp, ret};
use crate::lang::traits::free::FreeIds;
use crate::pass::structure::pretty::revive_underscore;
#[test]
fn test_revival_avoids_free_capture_and_agrees_with_fallback() {
    let (exps_match, block, block_else) = revive_underscore::apply_rel(
        &mut false,
        (vec![id_exp("_x")], vec![ret("_x"), ret("x")], Some(vec![ret("_x")])),
    )
    .unwrap();
    let id = exps_match[0].free_ids().iter().next().unwrap().clone();
    assert_ne!(id.node, "_x");
    assert_ne!(id.node, "x");
    assert!(block[0].free_ids().contains(&id));
    assert!(block_else.unwrap()[0].free_ids().contains(&id));
}

use super::super::{id, instr, signature, span};
use super::{binding, id_of_exp, iterator, return_exp};
use crate::lang::{common::notation::mixfix::Mixfix, hints::input::InputHint, il::ast::Iter};
use crate::pass::structure::ol::ast::*;
#[test]
fn test_unused_inputs_stay_underscored_and_let_scope_shadows_input() {
    let (exps_match, block, _) = revive_underscore::apply_rel(
        &mut false,
        (vec![id_exp("_unused"), id_exp("_x")], vec![binding("_x", "_x", vec![ret("_x")])], None),
    )
    .unwrap();
    assert_eq!(id_of_exp(&exps_match[0]).node, "_unused");
    let id_input = id_of_exp(&exps_match[1]);
    let InstrKind::Let(instr_let) = &block[0].node else { panic!("expected let") };
    assert_eq!(id_of_exp(&instr_let.exp_r), id_input);
    assert_ne!(id_of_exp(&instr_let.exp_l).node, id_input.node);
    assert_eq!(id_of_exp(return_exp(&instr_let.block[0])), id_of_exp(&instr_let.exp_l));
}

#[test]
fn test_rule_iterator_scopes_and_used_guard_survive_nested_branches() {
    let instr_rule = instr(InstrKind::Rule(RuleInstr {
        id: id("rel"),
        not_exp: Mixfix::Seq(vec![Mixfix::Arg(id_exp("_input")), Mixfix::Arg(id_exp("_out"))]),
        input_hint: InputHint::new(vec![0]),
        iter_instrs: vec![iterator("_input", "_out")],
        block: vec![ret("_out"), ret("_input")],
    }));
    let instr_case = instr(InstrKind::Case(CaseInstr {
        exp: id_exp("_input"),
        total: true,
        cases: vec![Case { guard: Guard::Mem(id_exp("_input")), block: vec![instr_rule] }],
    }));
    let instr_hold = instr(InstrKind::Hold(HoldInstr {
        id: id("rel"),
        not_exp: Mixfix::Arg(id_exp("_input")),
        iter_exps: vec![],
        block_hold: vec![instr_case],
        block_not_hold: vec![ret("_input")],
    }));
    let instr_group = instr(InstrKind::Group(GroupInstr {
        id: id("group"),
        rel_signature: signature(),
        exps: vec![id_exp("_input")],
        block: vec![instr_hold],
    }));
    let (exps_match, block, _) =
        revive_underscore::apply_rel(&mut false, (vec![id_exp("_input")], vec![instr_group], None))
            .unwrap();
    let id_input = id_of_exp(&exps_match[0]);
    let InstrKind::Group(instr_group) = &block[0].node else { panic!("expected group") };
    assert_eq!(id_of_exp(&instr_group.exps[0]), id_input);
    let InstrKind::Hold(instr_hold) = &instr_group.block[0].node else { panic!("expected hold") };
    assert_eq!(id_of_exp(return_exp(&instr_hold.block_not_hold[0])), id_input);
    let InstrKind::Case(instr_case) = &instr_hold.block_hold[0].node else {
        panic!("expected case")
    };
    let Guard::Mem(exp_guard) = &instr_case.cases[0].guard else { panic!("expected guard") };
    assert_eq!(id_of_exp(exp_guard), id_input);
    let InstrKind::Rule(instr_rule) = &instr_case.cases[0].block[0].node else {
        panic!("expected rule")
    };
    let exps = instr_rule.not_exp.args();
    let id_output = id_of_exp(exps[1]);
    assert_eq!(id_of_exp(exps[0]), id_input);
    assert_eq!(&instr_rule.iter_instrs[0].vars_bound[0].id, id_input);
    assert_eq!(&instr_rule.iter_instrs[0].vars_bind[0].id, id_output);
    assert_eq!(id_of_exp(return_exp(&instr_rule.block[0])), id_output);
    assert_ne!(id_output.node, "_out");
    assert_eq!(block[0].span, span(1));
}

#[test]
fn test_input_hint_failure_keeps_rule_span() {
    let mut instr_rule = instr(InstrKind::Rule(RuleInstr {
        id: id("rel"),
        not_exp: Mixfix::Arg(id_exp("_x")),
        input_hint: InputHint::new(vec![2]),
        iter_instrs: vec![],
        block: vec![],
    }));
    instr_rule.span = span(17);
    let error =
        revive_underscore::apply_rel(&mut false, (vec![], vec![instr_rule], None)).unwrap_err();
    assert_eq!(error.span, span(17));
    assert!(matches!(error.kind, crate::pass::structure::StructureErrorKind::Input(_)));
}

#[test]
fn test_let_iterator_bound_and_binding_names_follow_separate_scopes() {
    let instr_let = instr(InstrKind::Let(LetInstr {
        exp_l: id_exp("_local"),
        exp_r: id_exp("_input"),
        iter_instrs: vec![iterator("_input", "_local")],
        block: vec![ret("_local"), binding("_nested", "_local", vec![ret("_nested")])],
    }));
    let (exps_match, block, _) =
        revive_underscore::apply_rel(&mut false, (vec![id_exp("_input")], vec![instr_let], None))
            .unwrap();
    let id_input = id_of_exp(&exps_match[0]);
    let InstrKind::Let(instr_let) = &block[0].node else { panic!("expected let") };
    let id_local = id_of_exp(&instr_let.exp_l);
    assert_eq!(&instr_let.iter_instrs[0].vars_bound[0].id, id_input);
    assert_eq!(&instr_let.iter_instrs[0].vars_bind[0].id, id_local);
    assert_eq!(id_of_exp(return_exp(&instr_let.block[0])), id_local);
    let InstrKind::Let(instr_nested) = &instr_let.block[1].node else {
        panic!("expected nested let")
    };
    assert_eq!(id_of_exp(&instr_nested.exp_r), id_local);
    // Upstream revival stops at a binding and scans its body downstream
    assert_eq!(id_of_exp(&instr_nested.exp_l).node, "_nested");
    assert_eq!(id_of_exp(return_exp(&instr_nested.block[0])).node, "_nested");
}

#[test]
fn test_progress_tracks_iterator_only_uses_but_not_unused_candidates() {
    let mut changed = false;
    let body = (vec![id_exp("_x")], vec![ret("untouched")], None);
    assert_eq!(revive_underscore::apply_rel(&mut changed, body.clone()).unwrap(), body);
    assert!(!changed);

    let instr_if = instr(InstrKind::If(IfInstr {
        exp: id_exp("condition"),
        iter_exps: vec![ExpIter { iter: Iter::List, vars: iterator("_x", "unused").vars_bound }],
        block: vec![],
    }));
    let (exps_match, block, _) =
        revive_underscore::apply_rel(&mut changed, (vec![id_exp("_x")], vec![instr_if], None))
            .unwrap();
    assert!(changed);
    assert_eq!(id_of_exp(&exps_match[0]).node, "_x");
    let InstrKind::If(instr_if) = &block[0].node else { unreachable!() };
    assert_eq!(instr_if.iter_exps[0].vars[0].id.node, "x");
}
