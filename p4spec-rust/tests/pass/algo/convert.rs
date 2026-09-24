use p4spec_rust::pass::{algo, elaborate};
use p4spec_rust::{
    diagnostic::{LabelStyle, Report, ReportKind, Severity},
    lang::{
        al,
        common::{
            notation::mixfix::Mixfix,
            source::{Position, Span},
        },
        hints::input::InputHint,
        il::ast,
    },
    note_phrase, phrase,
};

#[test]
fn test_conversion_rejects_overlapping_crossed_alias_table_rows() {
    let source = r#"
syntax typeIR
syntax typeId = text
syntax typedefTypeIR = TYPEDEF typeId typeIR
syntax intTypeIR = INT
syntax typeIR =
  | intTypeIR
  | typedefTypeIR

tbl dec $compat(typeIR, typeIR) : bool
tbl def $compat =
  | (INT, INT) => true
  | (TYPEDEF _ typeIR_l, typeIR_r) => true
  | (typeIR_l, TYPEDEF _ typeIR_r) => true
  | (_, _) => false
"#;
    let spec_el = crate::spec_fixture::parse(source).expect("parse crossed alias table");
    let spec_il = elaborate::convert(spec_el).expect("elaborate crossed alias table");

    let error = algo::convert(spec_il).expect_err("crossed alias rows overlap by syntax");

    assert!(error.to_string().contains("algo/table-pattern-overlapping"));
}

fn cause(report: &Report) -> &p4spec_rust::diagnostic::Diagnostic {
    let ReportKind::Cause(diagnostic) = &report.kind else {
        panic!("expected terminal algorithmic cause");
    };
    assert_eq!(diagnostic.source, "algo");
    assert_eq!(diagnostic.severity, Severity::Error);
    diagnostic
}

#[test]
fn test_otherwise_diagnostics_relate_the_marker_in_functions_and_relations() {
    for (source, line_else, line_condition) in [
        ("dec $f : nat\ndef $f = 0\n  -- otherwise\n  -- if true\n", 3, 4),
        (
            "relation R: nat |- nat\n  hint(input %0)\nrule R/else: 0 |- 0\n  -- otherwise\n  -- if true\n",
            4,
            5,
        ),
    ] {
        let spec_el = crate::spec_fixture::parse(source).unwrap();
        let spec_il = elaborate::convert(spec_el).unwrap();
        let report = algo::convert(spec_il).unwrap_err();
        let diagnostic = cause(&report);
        assert_eq!(diagnostic.code.as_deref(), Some("algo/otherwise-condition-invalid"));
        assert_eq!(diagnostic.labels[0].style, LabelStyle::Primary);
        assert_eq!(diagnostic.labels[0].span.left.line, line_condition);
        let label_else = diagnostic
            .labels
            .iter()
            .find(|label| label.style == LabelStyle::Secondary)
            .expect("otherwise marker must be related to the forbidden operation");
        assert_eq!(label_else.span.left.line, line_else);
        assert_eq!(label_else.span.left.column, 5);
        assert_eq!(label_else.span.right.line, line_else);
        assert_eq!(label_else.span.right.column, 14);
    }
}

#[test]
fn test_synthesized_otherwise_clause_retains_its_enclosing_location() {
    let spec_el =
        crate::spec_fixture::parse("dec $f : nat\ndef $f = 0\n  -- otherwise\n  -- if true\n")
            .unwrap();
    let mut spec_il = elaborate::convert(spec_el).unwrap();
    let ast::DefKind::MetaFunc(ast::MetaFuncDef::Defined(func_il)) = &mut spec_il[0].node else {
        panic!("expected defined function");
    };
    let clause_il = func_il.else_clause.as_mut().unwrap();
    clause_il.node.otherwise_opt = None;
    let span_clause = clause_il.span.clone();
    let report = algo::convert(spec_il).unwrap_err();
    let diagnostic = cause(&report);
    assert_eq!(diagnostic.code.as_deref(), Some("algo/otherwise-condition-invalid"));
    assert_eq!(diagnostic.labels[1].style, LabelStyle::Secondary);
    assert_eq!(diagnostic.labels[1].span, span_clause);
}

fn span(line: usize) -> Span {
    let pos = Position::new("overlap.watsup", line, 0);
    Span::new(pos.clone(), pos)
}

fn bool_exp(value: bool, line: usize) -> ast::Exp {
    note_phrase!(node: ast::ExpKind::Bool(value), note: ast::TypKind::Bool, span: span(line))
}

fn tuple_exp(exps: Vec<ast::Exp>, line: usize) -> ast::Exp {
    let typs = exps
        .iter()
        .map(|exp| phrase!(node: exp.note.as_ref().clone(), span: exp.span.clone()))
        .collect();
    note_phrase!(node: ast::ExpKind::Tuple(exps), note: ast::TypKind::Tuple(typs), span: span(line))
}

fn relation_spec(exps_by_rule: Vec<Vec<ast::Exp>>) -> ast::Spec {
    let id = phrase!(node: "R".to_owned(), span: span(1));
    let input_hint = InputHint::new(
        (0..exps_by_rule[0].len())
            .map(|idx| phrase!(node: idx, span: span(1)))
            .collect(),
    );
    let not_typ = phrase!(node: Mixfix::Seq(exps_by_rule[0].iter().map(|exp| {
        Mixfix::Arg(phrase!(node: exp.note.as_ref().clone(), span: exp.span.clone()))
    }).collect()), span: span(1));
    let rules = exps_by_rule
        .into_iter()
        .map(|exps| {
            phrase!(node: ast::RuleKind {
        id: id.clone(),
        not_exp: Mixfix::Seq(exps.into_iter().map(Mixfix::Arg).collect()),
        prems: vec![],
    }, span: span(1))
        })
        .collect();
    let rule_group_il = phrase!(node: ast::RuleGroupKind { id: id.clone(), rules }, span: span(1));
    vec![phrase!(node: ast::DefKind::Rel(ast::RelDef::Defined(Box::new(ast::DefinedRel {
        id, not_typ, input_hint, rule_groups: vec![rule_group_il], else_group: None, hints: vec![],
    }))), span: span(1))]
}

fn rule_group(spec: &al::ast::Spec) -> &al::ast::RuleGroupKind {
    let al::ast::DefKind::Rel(al::ast::RelDef::Defined(rel)) = &spec[0].node else {
        panic!("expected defined relation");
    };
    &rel.rule_groups[0].node
}

#[test]
fn test_structural_overlap_preserves_tuple_and_path_order() {
    let spec_il = relation_spec(vec![
        vec![tuple_exp(vec![bool_exp(true, 2), bool_exp(false, 3)], 2)],
        vec![tuple_exp(vec![bool_exp(false, 5), bool_exp(true, 6)], 5)],
    ]);
    let spec_al = algo::convert(spec_il).unwrap();
    let rule_group_al = rule_group(&spec_al);
    let ast::ExpKind::Tuple(exps) = &rule_group_al.rule_match.exps_signature[0].node else {
        panic!("expected structural tuple template");
    };
    assert!(matches!(&exps[0].node, ast::ExpKind::Id(id) if id.node == "bool"));
    assert!(matches!(&exps[1].node, ast::ExpKind::Id(id) if id.node == "bool'"));
    for (rule_path, values) in rule_group_al
        .rule_paths
        .iter()
        .zip([[true, false], [false, true]])
    {
        assert_eq!(rule_path.prems.len(), 2);
        for (prem, value) in rule_path.prems.iter().zip(values) {
            let al::ast::PremKind::If(prem) = &prem.node else {
                panic!("expected comparison");
            };
            let ast::ExpKind::Cmp(_, _, _, exp_r) = &prem.exp.node else {
                panic!("expected equality");
            };
            assert_eq!(exp_r.node, ast::ExpKind::Bool(value));
        }
    }
}

#[test]
fn test_structural_fallback_discards_partial_fresh_names() {
    let exp_a = tuple_exp(vec![bool_exp(true, 2), bool_exp(false, 3)], 2);
    let mut exp_b = tuple_exp(vec![bool_exp(false, 5), bool_exp(true, 6)], 5);
    let ast::ExpKind::Tuple(exps) = &mut exp_b.node else { unreachable!() };
    // Force a late structural mismatch while the enclosing types still agree
    exps[1] = note_phrase!(node: ast::ExpKind::Num(ast::Num::Nat(0_u64.into())),
        note: ast::TypKind::Num(p4spec_rust::lang::common::prim::num::Typ::Nat), span: span(6));
    let spec_il =
        relation_spec(vec![vec![exp_a, bool_exp(true, 7)], vec![exp_b, bool_exp(false, 8)]]);
    let spec_al = algo::convert(spec_il).unwrap();
    let rule_group_al = rule_group(&spec_al);
    assert!(matches!(&rule_group_al.rule_match.exps_signature[0].node, ast::ExpKind::Id(_)));
    // The failed tuple attempt must not consume the fresh boolean name
    assert!(
        matches!(&rule_group_al.rule_match.exps_signature[1].node, ast::ExpKind::Id(id) if id.node == "bool")
    );
    assert_eq!(rule_group_al.rule_paths.len(), 2);
}

#[test]
fn test_nested_type_failure_is_not_retried_as_structural_mismatch() {
    let mut exp_a = tuple_exp(vec![bool_exp(true, 2), bool_exp(false, 3)], 2);
    let exp_b = tuple_exp(vec![bool_exp(false, 5), bool_exp(true, 6)], 5);
    let ast::ExpKind::Tuple(exps) = &mut exp_a.node else { unreachable!() };
    let id_missing = phrase!(node: "Missing".to_owned(), span: span(3));
    exps[1].note = ast::TypKind::Var(id_missing, vec![]).into();
    let spec_il = relation_spec(vec![vec![exp_a], vec![exp_b]]);
    let report = algo::convert(spec_il).unwrap_err();
    let diagnostic = cause(&report);
    assert_eq!(diagnostic.code.as_deref(), Some("algo/type-operation-invalid"));
    assert!(diagnostic.message.contains("Missing"));
    assert_eq!(diagnostic.labels[0].span, span(3));
    // Conversion carries no fresh-name state into a subsequent invocation
    let spec_il = relation_spec(vec![vec![bool_exp(true, 2)], vec![bool_exp(false, 3)]]);
    let spec_al = algo::convert(spec_il).unwrap();
    assert!(matches!(&rule_group(&spec_al).rule_match.exps_signature[0].node,
        ast::ExpKind::Id(id) if id.node == "bool"));
}

#[test]
fn test_incompatible_rule_inputs_report_a_located_failure() {
    let exp_nat = note_phrase!(node: ast::ExpKind::Num(ast::Num::Nat(0_u64.into())),
        note: ast::TypKind::Num(p4spec_rust::lang::common::prim::num::Typ::Nat), span: span(5));
    let report =
        algo::convert(relation_spec(vec![vec![bool_exp(true, 2)], vec![exp_nat]])).unwrap_err();
    let diagnostic = cause(&report);
    assert_eq!(diagnostic.code.as_deref(), Some("algo/rule-input-mismatch"));
    assert_eq!(diagnostic.labels[0].span, span(5));
}

#[test]
fn test_wildcards_remain_independent_table_positions() {
    let spec_el = crate::spec_fixture::parse(
        "syntax choice = A | B\ntbl dec $f(choice, choice) : bool\ntbl def $f = | (_, _) => true\n",
    )
    .unwrap();
    let spec_il = elaborate::convert(spec_el).unwrap();
    let spec_al = algo::convert(spec_il).unwrap();
    let func = spec_al
        .iter()
        .find_map(|def| match &def.node {
            al::ast::DefKind::MetaFunc(al::ast::MetaFuncDef::Table(func)) => Some(func),
            _ => None,
        })
        .unwrap();
    let row = &func.table_rows[0].node;
    assert!(row.prems.is_empty());
    let ids: Vec<_> = row
        .args
        .iter()
        .map(|arg| {
            let ast::ArgKind::Exp(exp) = &arg.node else { panic!("expected expression argument") };
            let ast::ExpKind::Id(id) = &exp.node else { panic!("expected wildcard variable") };
            &id.node
        })
        .collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
}

#[test]
fn test_table_repeated_binders_use_occurrence_order_and_related_span() {
    let spec_el = crate::spec_fixture::parse("syntax choice = A | B\nvar x : choice\nvar y : choice\ntbl dec $f(choice, choice, choice, choice) : bool\ntbl def $f = | (x, y, y, x) => true\n").unwrap();
    let report = algo::convert(elaborate::convert(spec_el).unwrap()).unwrap_err();
    let diagnostic = cause(&report);
    assert_eq!(diagnostic.code.as_deref(), Some("algo/table-binding-repeated"));
    assert!(diagnostic.message.contains("`y`"));
    assert_eq!(diagnostic.labels[0].style, LabelStyle::Primary);
    assert_eq!(diagnostic.labels[1].style, LabelStyle::Secondary);
    assert!(diagnostic.labels[0].span.left.column > diagnostic.labels[1].span.left.column);
}

fn upcast_table_patterns(mut spec: ast::Spec) -> ast::Spec {
    for def in &mut spec {
        let ast::DefKind::MetaFunc(ast::MetaFuncDef::Table(func)) = &mut def.node else { continue };
        for row in &mut func.rows {
            for arg in &mut row.node.args {
                let ast::ArgKind::Exp(exp) = &mut arg.node else { continue };
                if matches!(exp.node, ast::ExpKind::Case(_)) {
                    let typ = phrase!(node: exp.note.as_ref().clone(), span: exp.span.clone());
                    exp.node = ast::ExpKind::UpCast(Box::new(typ), exp.clone());
                }
            }
        }
    }
    spec
}

#[test]
fn test_upcast_case_keeps_bound_literal_patterns() {
    let spec_el = crate::spec_fixture::parse("syntax choice = SOME nat | NONE\ntbl dec $f(choice) : bool\ntbl def $f = | SOME 0 => true | _ => false\n").unwrap();
    let spec_il = elaborate::convert(spec_el).unwrap();
    algo::convert(upcast_table_patterns(spec_il))
        .expect("upcast cases retain bound literal arguments");
}

#[test]
fn test_upcast_case_rejects_repeated_nested_binders_before_renaming() {
    let spec_el = crate::spec_fixture::parse("syntax choice = BOX (nat, nat) | NONE\nvar x : nat\ntbl dec $f(choice) : bool\ntbl def $f = | BOX (x, x) => true | _ => false\n").unwrap();
    let spec_il = elaborate::convert(spec_el).unwrap();
    let report = algo::convert(upcast_table_patterns(spec_il)).unwrap_err();
    assert_eq!(cause(&report).code.as_deref(), Some("algo/table-binding-repeated"));
}

#[test]
fn test_direct_il_wildcard_closer_validates_row_arity() {
    let spec_el = crate::spec_fixture::parse(
        "syntax choice = A | B\ntbl dec $f(choice, choice) : bool\ntbl def $f = | (_, _) => true\n",
    )
    .unwrap();
    let mut spec_il = elaborate::convert(spec_el).unwrap();
    for def in &mut spec_il {
        if let ast::DefKind::MetaFunc(ast::MetaFuncDef::Table(func)) = &mut def.node {
            func.rows[0].node.args.pop();
        }
    }
    let report =
        algo::convert(spec_il).expect_err("a wildcard closer must match the declared arity");
    assert_eq!(cause(&report).code.as_deref(), Some("algo/table-pattern-arity-mismatch"));
}

#[test]
fn test_non_invertible_table_argument_keeps_its_diagnostic() {
    let spec_el = crate::spec_fixture::parse("syntax choice = BOX nat | NONE\nvar x : nat\ntbl dec $f(choice) : bool\ntbl def $f = | BOX x => true | _ => false\n").unwrap();
    let mut spec_il = elaborate::convert(spec_el).unwrap();
    for def in &mut spec_il {
        let ast::DefKind::MetaFunc(ast::MetaFuncDef::Table(func)) = &mut def.node else { continue };
        let ast::ArgKind::Exp(exp) = &mut func.rows[0].node.args[0].node else { unreachable!() };
        let ast::ExpKind::Case(not_exp) = &mut exp.node else { unreachable!() };
        **not_exp = not_exp.map(|exp| {
            note_phrase!(
            node: ast::ExpKind::Cat(Box::new(exp.clone()), Box::new(exp.clone())),
            note: exp.note.clone(), span: exp.span.clone())
        });
    }
    let report = algo::convert(upcast_table_patterns(spec_il)).unwrap_err();
    assert_eq!(cause(&report).code.as_deref(), Some("algo/binding-non-invertible"));
}
