use super::super::*;
use crate::lang::traits::eq::SyntaxEq;
#[test]
fn test_equality_preserves_order_total_and_ignores_spans_and_subproofs() {
    let typ = crate::phrase! { node: TypKind::Bool, span: span(1) };
    let case = ast_ol::Case {
        guard: Guard::Sub(typ.clone(), Box::new(Subcheck::Skip)),
        block: vec![ret("a"), ret("b")],
    };
    let instr_a = instr(ast_ol::InstrKind::Case(ast_ol::CaseInstr {
        exp: variable("x"),
        cases: vec![case],
        total: false,
    }));
    let mut instr_b = instr_a.clone();
    instr_b.span = span(2);
    let ast_ol::InstrKind::Case(instr_case) = &mut instr_b.node else {
        panic!("expected case")
    };
    instr_case.cases[0].guard = Guard::Sub(typ.clone(), Box::new(Subcheck::Recurse(typ)));
    assert_ne!(instr_a, instr_b);
    assert!(instr_a.syntax_eq(&instr_b));
    let ast_ol::InstrKind::Case(instr_case) = &mut instr_b.node else {
        panic!("expected case")
    };
    instr_case.total = true;
    assert!(!instr_a.syntax_eq(&instr_b));
    let ast_ol::InstrKind::Case(instr_case) = &mut instr_b.node else {
        panic!("expected case")
    };
    instr_case.total = false;
    instr_case.cases[0].block.reverse();
    assert!(!instr_a.syntax_eq(&instr_b));
}
#[test]
fn test_case_branch_order_matters() {
    let case_a = ast_ol::Case {
        guard: Guard::Bool(true),
        block: vec![ret("a")],
    };
    let case_b = ast_ol::Case {
        guard: Guard::Bool(false),
        block: vec![ret("b")],
    };
    assert!(![case_a.clone(), case_b.clone()].syntax_eq(&[case_b, case_a]));
}
