use super::super::*;
use crate::lang::traits::print::Print;
#[test]
fn test_nested_case_let_group_and_debug_render_source_layout() {
    let instr_group = instr(ast_ol::InstrKind::Group(ast_ol::GroupInstr {
        id: id("group"),
        rel_signature: signature(),
        exps: vec![variable("x")],
        block: vec![ret("x")],
    }));
    let instr_debug = instr(ast_ol::InstrKind::Debug(ast_ol::DebugInstr {
        exp: variable("trace"),
        instr: Box::new(instr_group),
    }));
    let instr_let = instr(ast_ol::InstrKind::Let(ast_ol::LetInstr {
        exp_l: variable("x"),
        exp_r: variable("y"),
        iter_instrs: vec![],
        block: vec![instr_debug],
    }));
    let block = vec![instr(ast_ol::InstrKind::Case(ast_ol::CaseInstr {
        exp: variable("subject"),
        cases: vec![ast_ol::Case {
            guard: Guard::Mem(variable("members")),
            block: vec![instr_let],
        }],
        total: true,
    }))];
    assert_eq!(
        block.to_string(),
        "1. Case analysis on subject\n\n  1. Case (% in members)\n\n    1. (Let x be y)\n\n      1. Debug: trace\n0. Group group: x\n\n  1. Return x"
    );
}

#[test]
fn test_hold_keeps_else_number_and_result_uses_output_positions() {
    let rel_signature = RelSignature {
        not_typ: crate::phrase! { node: Mixfix::Seq(vec![Mixfix::Arg(crate::phrase! { node: TypKind::Bool, span: span(1) }), Mixfix::Arg(crate::phrase! { node: TypKind::Bool, span: span(1) })]), span: span(1) },
        input_hint: InputHint::new(vec![0]),
    };
    let instr_result = instr(ast_ol::InstrKind::Result(ast_ol::ResultInstr {
        rel_signature,
        exps: vec![variable("output")],
    }));
    let block = vec![instr(ast_ol::InstrKind::Hold(ast_ol::HoldInstr {
        id: id("relation"),
        not_exp: Mixfix::Arg(variable("input")),
        iter_exps: vec![],
        block_hold: vec![instr_result],
        block_not_hold: vec![ret("fallback")],
    }))];
    assert_eq!(
        block.to_string(),
        "1. If (relation: input) holds, then\n\n  1. Result in % output\n\n1. Else,\n\n  1. Return fallback"
    );
}
