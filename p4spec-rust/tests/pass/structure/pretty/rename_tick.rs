use super::super::{ret, variable};
use crate::lang::traits::free::Free;
use crate::pass::structure::pretty::{RelBody, rename_tick};
#[test]
fn test_ticks_fill_smallest_available_gap() {
    let body = rename_tick::apply_rel(RelBody {
        exps_match: vec![variable("x'''")],
        block: vec![ret("x'''"), ret("x"), ret("x''")],
        block_else: Some(vec![ret("x'''")]),
    })
    .unwrap();
    let id = body.exps_match[0].free().iter().next().unwrap().clone();
    assert_eq!(id.node, "x'");
    assert!(body.block[0].free().contains(&id));
    assert!(body.block_else.unwrap()[0].free().contains(&id));
}

use super::super::{id, instr, span};
use super::{binding, iterator, return_exp, var_id};
use crate::lang::{common::notation::mixfix::Mixfix, hints::input::InputHint};
use crate::pass::structure::ol::ast::*;
#[test]
fn test_nested_bindings_avoid_upstream_guard_names_and_keep_iterator_roles() {
    let instr_let = instr(InstrKind::Let(LetInstr {
        exp_l: variable("x'''"),
        exp_r: variable("source"),
        iter_instrs: vec![iterator("x'''", "x'''")],
        block: vec![
            ret("x'''"),
            binding("x''''", "x'''", vec![ret("x''''"), ret("x'''")]),
        ],
    }));
    let instr_case = instr(InstrKind::Case(CaseInstr {
        exp: variable("case"),
        cases: vec![Case {
            guard: Guard::Mem(variable("x")),
            block: vec![instr_let],
        }],
        total: false,
    }));
    let body = rename_tick::apply_rel(RelBody {
        exps_match: vec![variable("x'")],
        block: vec![instr_case],
        block_else: None,
    })
    .unwrap();
    let InstrKind::Case(instr_case) = &body.block[0].node else {
        panic!("expected case")
    };
    let InstrKind::Let(instr_let) = &instr_case.cases[0].block[0].node else {
        panic!("expected let")
    };
    let id_bind = var_id(&instr_let.exp_l);
    assert_eq!(id_bind.node, "x''");
    assert_eq!(instr_let.iter_instrs[0].vars_bound[0].id.node, "x'''");
    assert_eq!(&instr_let.iter_instrs[0].vars_bind[0].id, id_bind);
    assert_eq!(var_id(return_exp(&instr_let.block[0])), id_bind);
    let InstrKind::Let(instr_inner) = &instr_let.block[1].node else {
        panic!("expected nested let")
    };
    assert_eq!(var_id(&instr_inner.exp_r), id_bind);
    assert_eq!(var_id(&instr_inner.exp_l).node, "x'''");
    assert_eq!(var_id(return_exp(&instr_inner.block[1])), id_bind);
}
#[test]
fn test_rule_output_renaming_keeps_input_and_locations() {
    let mut exp_output = variable("out'''");
    let crate::lang::il::ast::ExpKind::Var(id_output) = &mut exp_output.node else {
        unreachable!()
    };
    id_output.span = span(23);
    let instr_rule = instr(InstrKind::Rule(RuleInstr {
        id: id("rel"),
        not_exp: Mixfix::Seq(vec![
            Mixfix::Arg(variable("input")),
            Mixfix::Arg(exp_output),
        ]),
        input_hint: InputHint::new(vec![0]),
        iter_instrs: vec![iterator("input", "out'''")],
        block: vec![ret("out'''")],
    }));
    let body = rename_tick::apply_rel(RelBody {
        exps_match: vec![],
        block: vec![instr_rule],
        block_else: None,
    })
    .unwrap();
    let InstrKind::Rule(instr_rule) = &body.block[0].node else {
        panic!("expected rule")
    };
    let exps = instr_rule.not_exp.args();
    assert_eq!(var_id(exps[0]).node, "input");
    assert_eq!(var_id(exps[1]).node, "out");
    assert_eq!(var_id(exps[1]).span, span(23));
    assert_eq!(var_id(return_exp(&instr_rule.block[0])), var_id(exps[1]));
    assert_eq!(return_exp(&instr_rule.block[0]).span, span(1));
}
