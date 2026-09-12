use super::*;
use crate::lang::{
    common::notation::mixfix::Mixfix,
    il::ast::{CmpOp, ListPattern, OpTyp, OptPattern, Pattern},
    xl::bool::CmpOp as BoolCmpOp,
};
use crate::pass::structure::opt::pre::matchify_if_eq_terminal::apply;
fn comparison(exp_l: Exp, exp_r: Exp, op: BoolCmpOp) -> Exp {
    crate::note_phrase! {node: ExpKind::Cmp(CmpOp::Bool(op), OpTyp::Bool, Box::new(exp_l), Box::new(exp_r)), note: TypKind::Bool, span: span(8)}
}
fn condition(exp: Exp) -> Instr {
    instr(InstrKind::If(IfInstr {
        exp,
        iter_exps: vec![(Iter::Opt, vec![])],
        block: vec![ret("body")],
    }))
}
#[test]
fn test_empty_option_list_and_terminal_both_sides() {
    for (exp_kind, pattern_eq, pattern_ne) in [
        (
            ExpKind::Opt(None),
            Pattern::Opt(OptPattern::None),
            Pattern::Opt(OptPattern::Some),
        ),
        (
            ExpKind::List(vec![]),
            Pattern::List(ListPattern::Nil),
            Pattern::List(ListPattern::Cons),
        ),
    ] {
        let exp_terminal = crate::note_phrase! {node: exp_kind, note: TypKind::Bool, span: span(2)};
        for (op, pattern) in [(BoolCmpOp::Eq, pattern_eq), (BoolCmpOp::Ne, pattern_ne)] {
            for reversed in [false, true] {
                let (exp_l, exp_r) = if reversed {
                    (exp_terminal.clone(), variable("x"))
                } else {
                    (variable("x"), exp_terminal.clone())
                };
                let block = apply(vec![condition(comparison(exp_l, exp_r, op))]);
                let InstrKind::If(instr_if) = &block[0].node else {
                    panic!("expected if")
                };
                assert_eq!(instr_if.exp.span, span(8));
                assert_eq!(
                    instr_if.exp.node,
                    ExpKind::Match(Box::new(variable("x")), pattern.clone())
                );
                assert_eq!(instr_if.iter_exps, vec![(Iter::Opt, vec![])]);
                assert_eq!(instr_if.block, vec![ret("body")]);
            }
        }
    }
}
#[test]
fn test_terminal_case_inequality_and_nonterminal_counterexample() {
    let not_exp = Mixfix::Atom(
        crate::phrase! {node: crate::lang::common::notation::atom::Atom::Keyword("TERM".into()), span: span(6)},
    );
    let exp_terminal = crate::note_phrase! {node: ExpKind::Case(Box::new(not_exp.clone())), note: TypKind::Bool, span: span(2)};
    for reversed in [false, true] {
        let (exp_l, exp_r) = if reversed {
            (exp_terminal.clone(), variable("x"))
        } else {
            (variable("x"), exp_terminal.clone())
        };
        let block = apply(vec![condition(comparison(exp_l, exp_r, BoolCmpOp::Ne))]);
        let InstrKind::If(instr_if) = &block[0].node else {
            panic!("expected if")
        };
        let ExpKind::Un(_, OpTyp::Bool, exp) = &instr_if.exp.node else {
            panic!("expected negation")
        };
        assert_eq!(exp.span, span(8));
        assert_eq!(
            exp.node,
            ExpKind::Match(
                Box::new(variable("x")),
                Pattern::Case(Box::new(not_exp.to_mixop()))
            )
        );
    }
    let exp_nonterminal = crate::note_phrase! {node: ExpKind::Case(Box::new(Mixfix::Arg(variable("payload")))), note: TypKind::Bool, span: span(2)};
    let instr_if = condition(comparison(variable("x"), exp_nonterminal, BoolCmpOp::Eq));
    assert_eq!(apply(vec![instr_if.clone()]), vec![instr_if]);
}
#[test]
fn test_debug_and_nested_expression_are_not_traversed() {
    let exp_none =
        crate::note_phrase! {node: ExpKind::Opt(None), note: TypKind::Bool, span: span(2)};
    let exp_cmp = comparison(variable("x"), exp_none, BoolCmpOp::Eq);
    let instr_debug = instr(InstrKind::Debug(DebugInstr {
        exp: exp_cmp.clone(),
        instr: Box::new(condition(exp_cmp.clone())),
    }));
    let exp_nested = crate::note_phrase! {node: ExpKind::Opt(Some(Box::new(exp_cmp))), note: TypKind::Bool, span: span(2)};
    let instr_if = condition(exp_nested);
    assert_eq!(
        apply(vec![instr_debug.clone(), instr_if.clone()]),
        vec![instr_debug, instr_if]
    );
}

#[test]
fn test_all_nested_blocks_and_rule_priority() {
    let exp_none =
        crate::note_phrase! {node: ExpKind::Opt(None), note: TypKind::Bool, span: span(2)};
    let exp_nil =
        crate::note_phrase! {node: ExpKind::List(vec![]), note: TypKind::Bool, span: span(3)};
    let instr_if = condition(comparison(exp_none, exp_nil.clone(), BoolCmpOp::Eq));
    let exp_match = crate::note_phrase! {node: ExpKind::Match(Box::new(exp_nil), Pattern::Opt(OptPattern::None)), note: TypKind::Bool, span: span(8)};
    assert_eq!(
        apply(vec![group(containers(vec![instr_if]))]),
        vec![group(containers(vec![condition(exp_match)]))]
    );
}

#[test]
fn test_terminal_case_equality_preserves_notation_and_notes() {
    let not_exp = Mixfix::Atom(
        crate::phrase! {node: crate::lang::common::notation::atom::Atom::Keyword("TERM".into()), span: span(6)},
    );
    let exp_terminal = crate::note_phrase! {node: ExpKind::Case(Box::new(not_exp.clone())), note: TypKind::Bool, span: span(2)};
    for reversed in [false, true] {
        let (exp_l, exp_r) = if reversed {
            (exp_terminal.clone(), variable("x"))
        } else {
            (variable("x"), exp_terminal.clone())
        };
        let block = apply(vec![condition(comparison(exp_l, exp_r, BoolCmpOp::Eq))]);
        let exp_match = crate::note_phrase! {node: ExpKind::Match(Box::new(variable("x")), Pattern::Case(Box::new(not_exp.to_mixop()))), note: TypKind::Bool, span: span(8)};
        assert_eq!(block, vec![condition(exp_match)]);
    }
}
