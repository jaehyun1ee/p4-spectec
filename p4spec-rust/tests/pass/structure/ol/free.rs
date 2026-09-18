use super::super::*;
use crate::lang::{common::ds::set::IdSet, traits::free::Free};
#[test]
fn test_let_patterns_and_both_hold_blocks_are_collected() {
    let instr_let = instr(ast_ol::InstrKind::Let(ast_ol::LetInstr {
        exp_l: variable("pattern"),
        exp_r: variable("source"),
        iter_instrs: vec![InstrIter {
            iter: Iter::List,
            vars_bound: vec![Var {
                id: id("metadata"),
                typ: crate::phrase! {node: TypKind::Bool, span: span(1)},
                iters: vec![],
            }],
            vars_bind: vec![],
        }],
        block: vec![ret("body")],
    }));
    let instr_hold = instr(ast_ol::InstrKind::Hold(ast_ol::HoldInstr {
        id: id("relation"),
        not_exp: Mixfix::Arg(variable("argument")),
        iter_exps: vec![],
        block_hold: vec![instr_let],
        block_not_hold: vec![ret("fallback")],
    }));
    let ids: IdSet = ["pattern", "source", "body", "argument", "fallback"]
        .into_iter()
        .map(id)
        .collect();
    assert_eq!(instr_hold.free(), ids);
}

#[test]
fn test_nested_collection_preserves_first_identifier_spans_and_existing_names() {
    let mut id_head = id("shared");
    id_head.span = span(3);
    let exp_head = crate::note_phrase!(node: crate::lang::il::ast::ExpKind::Id(id_head.clone()), note: TypKind::Bool, span: span(3));
    let instr_result = ast_ol::ResultInstr {
        rel_signature: signature(),
        exps: vec![variable("shared"), variable("result")],
    };
    let instr_result = instr(ast_ol::InstrKind::Result(instr_result));
    let instr_debug = ast_ol::DebugInstr { exp: variable("debug"), instr: Box::new(instr_result) };
    let instr_debug = instr(ast_ol::InstrKind::Debug(instr_debug));
    let instr_rule = ast_ol::RuleInstr {
        id: id("relation"),
        not_exp: Mixfix::Arg(variable("rule_arg")),
        input_hint: InputHint::new(vec![0]),
        iter_instrs: vec![],
        block: vec![instr_debug],
    };
    let instr_rule = instr(ast_ol::InstrKind::Rule(instr_rule));
    let instr_group = ast_ol::GroupInstr {
        id: id("group"),
        rel_signature: signature(),
        exps: vec![variable("group_arg")],
        block: vec![instr_rule],
    };
    let instr_group = instr(ast_ol::InstrKind::Group(instr_group));
    let case = ast_ol::Case { guard: Guard::Mem(variable("guard")), block: vec![instr_group] };
    let instr_case = ast_ol::CaseInstr { exp: variable("case"), cases: vec![case], total: false };
    let instr_case = instr(ast_ol::InstrKind::Case(instr_case));
    let instr_if = ast_ol::IfInstr { exp: exp_head, iter_exps: vec![], block: vec![instr_case] };
    let block = vec![instr(ast_ol::InstrKind::If(instr_if))];
    let ids = block.free();
    let texts: Vec<_> = ids.iter().map(|id| id.node.as_str()).collect();
    assert_eq!(texts, ["case", "debug", "group_arg", "guard", "result", "rule_arg", "shared"]);
    assert_eq!(ids.iter().find(|id| id.node == "shared").unwrap(), &id_head);

    let mut id_existing = id("shared");
    id_existing.span = span(8);
    let mut ids = IdSet::from([id_existing.clone(), id("existing")]);
    block.free_into(&mut ids);
    assert_eq!(ids.len(), 8);
    assert!(ids.contains(&id("existing")));
    assert_eq!(ids.iter().find(|id| id.node == "shared").unwrap(), &id_existing);
}
