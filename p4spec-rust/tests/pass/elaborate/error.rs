use std::path::Path;

use p4spec_rust::{
    diagnostic::{LabelStyle, RenderConfig, Renderer, Report, ReportKind, Severity},
    frontend::parse::{parse_files, parse_text},
    pass::elaborate,
};

#[test]
fn test_unmatched_variant_displays_its_notation_declaration() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/elaboration/unmatched_variant.watsup");
    let spec = parse_files([fixture]).expect("parse unmatched variant fixture");

    let error = elaborate::convert(spec).expect_err("reject unmatched variant fixture");
    let diagnostic = Renderer::new(RenderConfig::default())
        .render_to_string(&error)
        .unwrap();

    assert!(diagnostic.contains("expected 'YES', but found 'NO'"));
    assert!(diagnostic.contains("expected notation: YES"));
    assert!(!diagnostic.contains("trace["));
}

#[test]
fn test_dimension_mismatch_displays_the_conflicting_dimensions() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/elaboration/dimension_mismatch.watsup");
    let spec = parse_files([fixture]).expect("parse dimension mismatch fixture");

    let error = elaborate::convert(spec).expect_err("reject mismatched dimensions");
    let diagnostic = Renderer::new(RenderConfig::default())
        .render_to_string(&error)
        .unwrap();

    assert!(diagnostic.contains("`K_x`"));
    assert!(diagnostic.contains("K*"));
    assert!(diagnostic.contains("K?"));
}

fn cause(report: &Report) -> &p4spec_rust::diagnostic::Diagnostic {
    let ReportKind::Cause(diagnostic) = &report.kind else { panic!("expected cause") };
    diagnostic
}

fn find_cause<'a>(
    report: &'a Report,
    code: &str,
) -> Option<&'a p4spec_rust::diagnostic::Diagnostic> {
    if let ReportKind::Cause(diagnostic) = &report.kind
        && diagnostic.code.as_deref() == Some(code)
    {
        return Some(diagnostic);
    }
    report
        .children
        .iter()
        .find_map(|child| find_cause(child, code))
}

#[test]
fn test_declaration_location_survives_type_completion() {
    let spec_el =
        parse_text("type.watsup".into(), "syntax foo\nsyntax foo = nat\nsyntax foo = int\n")
            .unwrap();
    let report = elaborate::convert(spec_el).unwrap_err();
    let diagnostic = cause(&report);
    assert_eq!(diagnostic.code.as_deref(), Some("elab/type-definition-repeated"));
    assert_eq!(diagnostic.labels[0].style, LabelStyle::Primary);
    assert_eq!(diagnostic.labels[0].span.left.line, 3);
    assert_eq!(diagnostic.labels[1].style, LabelStyle::Secondary);
    assert_eq!(diagnostic.labels[1].span.left.line, 1);
}

#[test]
fn test_forward_definition_rejects_a_matching_suffixed_type_parameter() {
    let spec_el = parse_text(
        "suffixed-type-parameter.watsup".into(),
        "syntax foo<T_1>\nsyntax foo<T_1> = nat\n",
    )
    .unwrap();
    let report = elaborate::convert(spec_el).expect_err("reject suffixed type parameter");
    let diagnostic = find_cause(&report, "elab/type-parameter-identifier-invalid")
        .expect("invalid type parameter diagnostic");
    assert_eq!(diagnostic.labels[0].span.left.line, 2);
    assert_eq!(diagnostic.labels[0].span.left.column, 11);
}

#[test]
fn test_forward_type_parameter_mismatch_relates_the_original_parameter_list() {
    let spec_el = parse_text(
        "forward-parameters.watsup".into(),
        "syntax foo<T, U>\nsyntax foo<U, T> = nat\n",
    )
    .unwrap();
    let report = elaborate::convert(spec_el).expect_err("reject reordered type parameters");
    let diagnostic = find_cause(&report, "elab/type-parameter-mismatch")
        .expect("type parameter mismatch diagnostic");
    assert_eq!(diagnostic.labels[0].style, LabelStyle::Primary);
    assert_eq!(diagnostic.labels[0].span.left.line, 2);
    assert_eq!(diagnostic.labels[0].span.left.column, 11);
    assert_eq!(diagnostic.labels[0].span.right.column, 15);
    assert_eq!(diagnostic.labels[1].style, LabelStyle::Secondary);
    assert_eq!(diagnostic.labels[1].span.left.line, 1);
    assert_eq!(diagnostic.labels[1].span.left.column, 11);
    assert_eq!(diagnostic.labels[1].span.right.column, 15);
}

#[test]
fn test_population_warnings_survive_a_later_dimension_failure() {
    let spec_el = parse_text(
        "warning.watsup".into(),
        concat!(
            "syntax pending\n",
            "dec $missing : nat\n",
            "dec $same_dimension<K>(K*, K?) : bool\n",
            "def $same_dimension<K>(K_x*, K_x?) = true\n",
        ),
    )
    .unwrap();
    let (result, warnings) = elaborate::convert_with_warnings(spec_el);
    assert!(result.is_err());
    assert_eq!(warnings.len(), 2);
    let diagnostic = cause(&warnings[0]);
    assert_eq!(diagnostic.severity, Severity::Warning);
    assert_eq!(diagnostic.code.as_deref(), Some("elab/type-definition-missing"));
    assert_eq!(diagnostic.labels[0].span.left.line, 1);
    let diagnostic = cause(&warnings[1]);
    assert_eq!(diagnostic.severity, Severity::Warning);
    assert_eq!(diagnostic.code.as_deref(), Some("elab/function-clause-missing"));
    assert_eq!(diagnostic.labels[0].span.left.line, 2);
    let spec_el = parse_text("next.watsup".into(), "var n : nat\n").unwrap();
    let (result, warnings) = elaborate::convert_with_warnings(spec_el);
    assert!(result.is_ok());
    assert!(warnings.is_empty());
}

#[test]
fn test_table_return_accepts_a_boolean_alias() {
    let spec_el = parse_text(
        "alias.watsup".into(),
        "syntax flag = bool\ntbl dec $lookup(nat) : flag\ntbl def $lookup =\n  | _ => true\n",
    )
    .unwrap();
    let (result, warnings) = elaborate::convert_with_warnings(spec_el);
    assert!(result.is_ok(), "{result:?}");
    assert!(warnings.is_empty());
}

#[test]
fn test_contextual_failure_retains_the_undefined_function_cause() {
    let spec_el = parse_text(
        "undefined.watsup".into(),
        "var n : nat\ndec $caller(nat) : nat\ndef $caller(n) = $missing(n)\n",
    )
    .unwrap();
    let report = elaborate::convert(spec_el).unwrap_err();
    let mut reports = vec![report.as_ref()];
    let mut found = false;
    while let Some(report) = reports.pop() {
        if let ReportKind::Cause(diagnostic) = &report.kind
            && diagnostic.code.as_deref() == Some("elab/function-undefined")
        {
            assert_eq!(diagnostic.labels[0].style, LabelStyle::Primary);
            assert_eq!(diagnostic.labels[0].span.left.line, 3);
            found = true;
        }
        reports.extend(&report.children);
    }
    assert!(found, "contextual fallback discarded the inference diagnostic");
}

#[test]
fn test_fatal_variant_candidate_stops_before_later_cases() {
    let spec_el = parse_text(
        "variant-candidate.watsup".into(),
        concat!(
            "syntax choice =\n",
            "| nat GOOD\n",
            "| bool BAD\n",
            "dec $f : choice\n",
            "def $f = $missing() GOOD\n",
        ),
    )
    .unwrap();
    let report = elaborate::convert(spec_el).expect_err("reject undefined nested call");
    let diagnostic = cause(&report);
    assert_eq!(diagnostic.code.as_deref(), Some("elab/function-undefined"));
    assert_eq!(diagnostic.labels[0].span.left.line, 5);
    assert!(report.children.is_empty());
}

#[test]
fn test_hint_only_syntax_reports_its_direct_cause_once() {
    let spec_el = parse_text("unparen.watsup".into(), "dec $f : nat\ndef $f = ## 0\n").unwrap();
    let report = elaborate::convert(spec_el).expect_err("reject hint-only syntax");
    let diagnostic = cause(&report);
    assert_eq!(diagnostic.code.as_deref(), Some("elab/unparen-outside-hint-unsupported"));
    assert_eq!(diagnostic.notes.len(), 1);
    assert!(report.children.is_empty());

    let rendered = Renderer::new(RenderConfig::default())
        .render_to_string(&report)
        .unwrap();
    assert_eq!(
        rendered
            .matches("unparenthesizing operator `##` is not allowed")
            .count(),
        1
    );
}

#[test]
fn test_missing_input_hint_warnings_commit_before_duplicate_admission() {
    let spec_el = parse_text(
        "duplicate-relation.watsup".into(),
        "relation R: nat |- nat\nrelation R: nat |- nat\n",
    )
    .unwrap();
    let (result, warnings) = elaborate::convert_with_warnings(spec_el);
    let report = result.expect_err("reject duplicate relation declaration");
    assert!(find_cause(&report, "elab/relation-repeated").is_some());
    assert_eq!(warnings.len(), 2);
    for (line, warning) in (1..=2).zip(&warnings) {
        let diagnostic = cause(warning);
        assert_eq!(diagnostic.code.as_deref(), Some("elab/relation-input-hint-missing"));
        assert_eq!(diagnostic.labels[0].span.left.line, line);
    }
}

#[test]
fn test_missing_definition_warning_order_matches_elaboration_phases() {
    let spec_el = parse_text(
        "warning-order.watsup".into(),
        concat!("syntax z\n", "syntax a\n", "relation R: nat |- nat\n", "dec $missing : nat\n",),
    )
    .unwrap();
    let (result, warnings) = elaborate::convert_with_warnings(spec_el);
    assert!(result.is_ok(), "{result:?}");
    let codes = warnings
        .iter()
        .map(|report| cause(report).code.as_deref().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        codes,
        [
            "elab/relation-input-hint-missing",
            "elab/type-definition-missing",
            "elab/type-definition-missing",
            "elab/relation-rule-missing",
            "elab/function-clause-missing",
        ]
    );
    assert_eq!(cause(&warnings[1]).labels[0].span.left.line, 2);
    assert_eq!(cause(&warnings[2]).labels[0].span.left.line, 1);
}

#[test]
fn test_explicit_empty_input_hint_remains_invalid_for_zero_arity_relation() {
    let spec_el =
        parse_text("empty-input.watsup".into(), "relation R: _OK\n  hint(input)\n").unwrap();
    let report = elaborate::convert(spec_el).expect_err("reject explicit empty input hint");
    assert!(find_cause(&report, "elab/relation-input-hint-empty").is_some());
}

#[test]
fn test_function_argument_count_precedes_full_signature_mismatch() {
    let spec_el = parse_text(
        "function-signature.watsup".into(),
        concat!(
            "dec $passed<T>(T, T) : text\n",
            "dec $caller(def $expected<U, V>(nat) : nat) : nat\n",
            "dec $main : nat\n",
            "def $main = $caller(def $passed)\n",
        ),
    )
    .unwrap();
    let report = elaborate::convert(spec_el).expect_err("reject incompatible function argument");
    assert!(find_cause(&report, "elab/function-argument-type-parameter-arity-mismatch").is_some());
    assert!(find_cause(&report, "elab/function-argument-signature-mismatch").is_none());
}

#[test]
fn test_call_type_argument_count_precedes_type_elaboration() {
    let spec_el = parse_text(
        "call-type-arguments.watsup".into(),
        concat!(
            "dec $callee<T>(T) : T\n",
            "dec $caller : nat\n",
            "def $caller = $callee<missing, nat>(0)\n",
        ),
    )
    .unwrap();
    let report = elaborate::convert(spec_el).expect_err("reject extra call type argument");
    assert!(find_cause(&report, "elab/function-call-type-argument-arity-mismatch").is_some());
    assert!(find_cause(&report, "elab/type-undefined").is_none());
}

#[test]
fn test_update_path_bounds_fail_before_path_shape_mismatch() {
    for (name, path) in [("index", "n[[$missing()] = 0]"), ("slice", "n[[$missing():0] = 0]")] {
        let source = format!("var n : nat\ndec $f(nat) : nat\ndef $f(n) = {path}\n");
        let spec_el = parse_text(format!("{name}.watsup").into(), &source).unwrap();
        let report = elaborate::convert(spec_el).expect_err("reject undefined path bound call");
        let diagnostic = cause(&report);
        assert_eq!(diagnostic.code.as_deref(), Some("elab/function-undefined"));
        assert_eq!(diagnostic.labels[0].span.left.line, 3);
        assert!(report.children.is_empty());
    }
}

#[test]
fn test_later_ordinary_rule_is_primary_when_otherwise_rule_comes_first() {
    let spec_el = parse_text(
        "otherwise-first.watsup".into(),
        concat!(
            "syntax foo = nat\n",
            "relation R: foo |- foo\n",
            "  hint(input %0)\n",
            "rulegroup R/group {\n",
            "  rule R/fallback:\n",
            "    0 |- 0\n",
            "    -- otherwise\n",
            "  rule R/regular:\n",
            "    0 |- 0\n",
            "}\n",
        ),
    )
    .unwrap();
    let report = elaborate::convert(spec_el).expect_err("reject mixed otherwise rule group");
    let diagnostic = find_cause(&report, "elab/relation-rule-otherwise-invalid")
        .expect("otherwise-group diagnostic");
    assert_eq!(diagnostic.labels[0].style, LabelStyle::Primary);
    assert_eq!(diagnostic.labels[0].span.left.line, 8);
    assert_eq!(diagnostic.labels[1].style, LabelStyle::Secondary);
    assert_eq!(diagnostic.labels[1].span.left.line, 5);
}
