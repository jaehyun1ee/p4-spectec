use crate::{
    lang::{
        al::ast::*,
        common::{
            ds::set::IdSet,
            source::{Position, Span},
        },
        traits::{eq::SyntaxEq, free::Free},
    },
    pass::structure::{
        StructureErrorKind,
        antiunify::{antiunify_clauses, antiunify_rule_match_group},
    },
};
fn span(int_line: i64) -> Span {
    let pos = Position::new("antiunify.watsup", int_line, 0);
    Span::new(pos.clone(), pos)
}
fn variable(text: &str, int_line: i64) -> Exp {
    let id = crate::phrase! {node: text.to_owned(), span: span(int_line)};
    crate::note_phrase! {node: ExpKind::Var(id), note: TypKind::Bool, span: span(int_line)}
}
fn boolean(value: bool, int_line: i64) -> Exp {
    crate::note_phrase! {node: ExpKind::Bool(value), note: TypKind::Bool, span: span(int_line)}
}
fn tuple(exps: Vec<Exp>, int_line: i64) -> Exp {
    crate::note_phrase! {node: ExpKind::Tuple(exps), note: TypKind::Bool, span: span(int_line)}
}
fn clause(exp_input: Exp, exp_output: Exp, prems: Vec<Prem>, int_line: i64) -> Clause {
    let arg = crate::phrase! {node: ArgKind::Exp(Box::new(exp_input)), span: span(int_line)};
    crate::phrase! {node: ClauseKind {args: vec![arg], exp: exp_output, prems}, span: span(int_line)}
}
fn let_prem(prem: &Prem) -> &LetPrem {
    let prem_kind = &prem.node;
    let PremKind::Let(prem_let) = prem_kind else {
        panic!("expected generated binding")
    };
    prem_let
}
#[test]
fn test_variable_and_tuple_preserve_template_and_binding_spans() {
    let exp_a = variable("x", 2);
    let exp_b = tuple(vec![boolean(true, 8)], 8);
    let mut frees = exp_a.free();
    exp_b.free_into(&mut frees);
    let group = antiunify_rule_match_group(
        frees.clone(),
        &[vec![exp_a.clone()], vec![exp_b.clone()]],
        None,
    )
    .unwrap();
    let exp_template = &group.exps_template[0];
    assert_eq!(exp_template.span, exp_a.span);
    assert_eq!(exp_template.note, exp_a.note);
    assert!(exp_template.free().iter().all(|id| !frees.contains(id)));
    for (prems, exp) in group.prems_group.iter().zip([exp_a, exp_b]) {
        let prem_let = let_prem(&prems[0]);
        assert_eq!(prem_let.exp_l, exp);
        assert_eq!(prem_let.exp_r, *exp_template);
        assert_eq!(
            prems[0].span,
            Span::over(&[exp.span, exp_template.span.clone()])
        );
    }
}
#[test]
fn test_else_clause_participates_and_generated_premises_precede_originals() {
    let prem_original =
        crate::phrase! {node: PremKind::Debug(DebugPrem {exp: variable("x'", 4)}), span: span(4)};
    let clause_a = clause(
        boolean(true, 2),
        variable("x''", 3),
        vec![prem_original.clone()],
        2,
    );
    let clause_else = clause(
        variable("x", 7),
        variable("x'''", 8),
        vec![prem_original.clone()],
        7,
    );
    let mut frees = clause_a.free();
    clause_else.free_into(&mut frees);
    let clauses = antiunify_clauses(vec![clause_a.clone()], Some(clause_else.clone())).unwrap();
    assert!(
        clauses.args_template[0]
            .free()
            .iter()
            .all(|id| !frees.contains(id))
    );
    let path = &clauses.paths[0];
    assert_eq!(let_prem(&path.prems[0]).exp_l, boolean(true, 2));
    assert_eq!(path.prems[1], prem_original);
    assert_eq!(path.exp, clause_a.node.exp);
    let path_else = clauses.path_else.unwrap();
    assert_eq!(path_else.prems[1], prem_original);
    assert_eq!(path_else.exp, clause_else.node.exp);
}
#[test]
fn test_incompatible_shapes_report_second_span() {
    let error = antiunify_rule_match_group(
        IdSet::new(),
        &[vec![boolean(true, 2)], vec![boolean(false, 9)]],
        None,
    )
    .unwrap_err();
    assert_eq!(error.kind, StructureErrorKind::Antiunification);
    assert_eq!(error.span, span(9));
}
#[test]
fn test_group_arity_mismatch_is_located() {
    let error = antiunify_rule_match_group(
        IdSet::new(),
        &[
            vec![boolean(true, 2)],
            vec![boolean(true, 9), boolean(false, 10)],
        ],
        None,
    )
    .unwrap_err();
    assert!(matches!(
        error.kind,
        StructureErrorKind::ArityMismatch {
            expected: 1,
            actual: 2
        }
    ));
    assert_eq!(error.span, Span::over(&[span(9), span(10)]));
}
#[test]
fn test_incompatible_definition_arguments_are_typed() {
    let mut clause_a = clause(boolean(true, 1), boolean(true, 2), vec![], 1);
    clause_a.node.args[0] = crate::phrase! {node: ArgKind::Def(crate::phrase! {node: "f".to_owned(), span: span(1)}), span: span(1)};
    let mut clause_b = clause_a.clone();
    clause_b.node.args[0] = crate::phrase! {node: ArgKind::Def(crate::phrase! {node: "g".to_owned(), span: span(9)}), span: span(9)};
    let error = antiunify_clauses(vec![clause_a], Some(clause_b)).unwrap_err();
    assert_eq!(error.kind, StructureErrorKind::IncompatibleArguments);
    assert_eq!(error.span, span(9));
}

fn record(exp: Exp, int_line: i64) -> Exp {
    let atom = crate::phrase! {node: crate::lang::common::notation::atom::Atom::Keyword("field".to_owned()), span: span(int_line)};
    crate::note_phrase! {node: ExpKind::Str(vec![(atom, exp)]), note: TypKind::Bool, span: span(int_line)}
}
fn case(exp: Exp, int_line: i64) -> Exp {
    use crate::lang::common::notation::mixfix::Mixfix;
    let atom = crate::phrase! {node: crate::lang::common::notation::atom::Atom::Keyword("TAG".to_owned()), span: span(int_line)};
    let not_exp = Mixfix::Seq(vec![Mixfix::Atom(atom), Mixfix::Arg(exp)]);
    crate::note_phrase! {node: ExpKind::Case(Box::new(not_exp)), note: TypKind::Bool, span: span(int_line)}
}
#[test]
fn test_structured_templates_populate_in_source_order() {
    let exp_a = tuple(
        vec![record(variable("x", 2), 2), case(variable("y", 3), 3)],
        1,
    );
    let exp_b = tuple(
        vec![record(boolean(true, 8), 8), case(boolean(false, 9), 9)],
        7,
    );
    let group = antiunify_rule_match_group(exp_a.free(), &[vec![exp_a.clone()], vec![exp_b]], None)
        .unwrap();
    assert_eq!(group.prems_group[1].len(), 2);
    assert_eq!(let_prem(&group.prems_group[1][0]).exp_l, boolean(true, 8));
    assert_eq!(let_prem(&group.prems_group[1][1]).exp_l, boolean(false, 9));
    let ExpKind::Tuple(exps_template) = &group.exps_template[0].node else {
        panic!("tuple template")
    };
    assert_eq!(exps_template[0].span, span(2));
    assert_eq!(exps_template[1].span, span(3));
}
#[test]
fn test_unified_identifier_is_retained_for_later_group_members() {
    let exp_a = variable("x", 1);
    let group = antiunify_rule_match_group(
        exp_a.free(),
        &[vec![exp_a], vec![boolean(true, 4)], vec![boolean(false, 8)]],
        None,
    )
    .unwrap();
    let exp_template = &group.exps_template[0];
    for prems in &group.prems_group {
        assert!(let_prem(&prems[0]).exp_r.syntax_eq(exp_template));
    }
}
#[test]
fn test_iterated_template_preserves_bound_and_binding_variables() {
    let exp_a = variable("x", 2);
    let exp_b = variable("y", 8);
    let var_a = Var {
        id: crate::phrase! {node: "x".to_owned(), span: span(2)},
        typ: crate::phrase! {node: TypKind::Bool, span: span(2)},
        iters: vec![],
    };
    let var_b = Var {
        id: crate::phrase! {node: "y".to_owned(), span: span(8)},
        typ: var_a.typ.clone(),
        iters: vec![],
    };
    let exp_iter_a = crate::note_phrase! {node: ExpKind::Iter(Box::new(exp_a), (Iter::List, vec![var_a.clone()])), note: TypKind::Bool, span: span(1)};
    let exp_iter_b = crate::note_phrase! {node: ExpKind::Iter(Box::new(exp_b), (Iter::List, vec![var_b.clone()])), note: TypKind::Bool, span: span(7)};
    let mut frees = exp_iter_a.free();
    exp_iter_b.free_into(&mut frees);
    let group =
        antiunify_rule_match_group(frees, &[vec![exp_iter_a], vec![exp_iter_b]], None).unwrap();
    let ExpKind::Iter(exp_template, (iter, vars_template)) = &group.exps_template[0].node else {
        panic!("iter template")
    };
    assert_eq!(*iter, Iter::List);
    assert_eq!(vars_template.len(), 1);
    assert!(exp_template.free().contains(&vars_template[0].id));
    for (prems, var) in group.prems_group.iter().zip([var_a, var_b]) {
        let prem_kind = &prems[0].node;
        let PremKind::Iter(prem_iter) = prem_kind else {
            panic!("iterated binding")
        };
        assert_eq!(prem_iter.prem_iter.vars_bound, *vars_template);
        assert_eq!(prem_iter.prem_iter.vars_bind, vec![var]);
        assert_eq!(
            prem_iter.prem.span,
            Span::over(&[span(2), let_prem(&prem_iter.prem).exp_l.span.clone()])
        );
    }
}

#[test]
fn test_nested_tuple_arity_is_located_at_second_tuple() {
    let exp_a = tuple(vec![boolean(true, 2)], 1);
    let exp_b = tuple(vec![], 9);
    let error =
        antiunify_rule_match_group(IdSet::new(), &[vec![exp_a], vec![exp_b]], None).unwrap_err();
    assert_eq!(
        error.kind,
        StructureErrorKind::ArityMismatch {
            expected: 1,
            actual: 0
        }
    );
    assert_eq!(error.span, span(9));
}

#[test]
fn test_empty_clause_arguments_report_clause_span() {
    let clause_a = clause(boolean(true, 1), boolean(true, 2), vec![], 1);
    let mut clause_b = clause_a.clone();
    clause_b.node.args.clear();
    clause_b.span = span(9);
    let error = antiunify_clauses(vec![clause_a, clause_b], None).unwrap_err();
    assert_eq!(
        error.kind,
        StructureErrorKind::ArityMismatch {
            expected: 1,
            actual: 0
        }
    );
    assert_eq!(error.span, span(9));
}

#[test]
fn test_rule_else_participates_and_preserves_its_bindings() {
    let exp_else = variable("x", 9);
    let group = antiunify_rule_match_group(
        exp_else.free(),
        &[vec![boolean(true, 1)]],
        Some(&[exp_else.clone()]),
    )
    .unwrap();
    assert_eq!(let_prem(&group.prems_else.unwrap()[0]).exp_l, exp_else);
    assert_eq!(let_prem(&group.prems_group[0][0]).exp_l, boolean(true, 1));
    assert_eq!(group.exps_template[0].span, span(1));
    let ExpKind::Var(id) = &group.exps_template[0].node else {
        panic!("variable template")
    };
    assert_eq!(id.span, span(9));
}

#[test]
fn test_freshness_accumulates_across_input_columns() {
    let exp_a = variable("x", 1);
    let exp_b = variable("x'", 2);
    let mut frees = exp_a.free();
    exp_b.free_into(&mut frees);
    let group = antiunify_rule_match_group(
        frees.clone(),
        &[
            vec![exp_a, exp_b],
            vec![boolean(true, 8), boolean(false, 9)],
        ],
        None,
    )
    .unwrap();
    let ids = group.exps_template.as_slice().free();
    assert_eq!(ids.len(), 2);
    assert!(ids.iter().all(|id| !frees.contains(id)));
    for exp in &group.exps_template {
        let ExpKind::Var(id) = &exp.node else {
            panic!("variable template")
        };
        assert_eq!(id.span, exp.span);
    }
}

#[test]
fn test_conflicting_input_column_unifiers_are_typed() {
    let exp = variable("x", 1);
    let error = antiunify_rule_match_group(
        exp.free(),
        &[
            vec![exp.clone(), exp],
            vec![boolean(true, 8), boolean(false, 9)],
        ],
        None,
    )
    .unwrap_err();
    assert_eq!(error.kind, StructureErrorKind::ConflictingUnification);
    assert_eq!(error.span, span(1));
}

#[test]
fn test_record_field_disagreement_is_located() {
    let exp_a = record(variable("x", 2), 1);
    let mut exp_b = record(boolean(true, 8), 9);
    let ExpKind::Str(expfields) = &mut exp_b.node else {
        panic!("record")
    };
    expfields[0].0.node = crate::lang::common::notation::atom::Atom::Keyword("other".to_owned());
    let error =
        antiunify_rule_match_group(exp_a.free(), &[vec![exp_a], vec![exp_b]], None).unwrap_err();
    assert_eq!(error.kind, StructureErrorKind::Antiunification);
    assert_eq!(error.span, span(9));
}

#[test]
fn test_equal_inputs_generate_no_premises_and_keep_original_annotations() {
    let exp_a = case(boolean(true, 2), 1);
    let exp_b = case(boolean(true, 8), 9);
    let group = antiunify_rule_match_group(IdSet::new(), &[vec![exp_a.clone()], vec![exp_b]], None)
        .unwrap();
    assert_eq!(group.exps_template, vec![exp_a]);
    assert!(group.prems_group.iter().all(Vec::is_empty));
}

#[test]
fn test_overwritten_unifier_cannot_silently_populate_an_old_template() {
    let exp_a = tuple(vec![variable("x", 2), variable("x", 3)], 1);
    let exp_b = tuple(vec![boolean(true, 8), boolean(false, 9)], 7);
    let error =
        antiunify_rule_match_group(exp_a.free(), &[vec![exp_a], vec![exp_b]], None).unwrap_err();
    assert_eq!(error.kind, StructureErrorKind::TemplatePopulation);
    assert_eq!(error.span, span(2));
}
