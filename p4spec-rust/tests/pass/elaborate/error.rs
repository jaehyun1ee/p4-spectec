use std::path::Path;

use p4spec_rust::{
    diagnostic::{LabelStyle, RenderConfig, Renderer, Report, ReportKind, Severity},
    frontend::parse::{parse_files, parse_text},
    pass::elaborate,
};

#[test]
fn test_backtracking_failure_displays_its_elaboration_trace() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/elaboration/unmatched_variant.watsup");
    let spec = parse_files([fixture]).expect("parse unmatched variant fixture");

    let error = elaborate::convert(spec).expect_err("reject unmatched variant fixture");
    let diagnostic = Renderer::new(RenderConfig::default())
        .render_to_string(&error)
        .unwrap();

    assert!(diagnostic.contains("expression elaboration failed"));
    assert!(diagnostic.contains("expression does not match any variant case"));
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
fn test_population_warnings_survive_a_later_dimension_failure() {
    let spec_el = parse_text(
        "warning.watsup".into(),
        concat!(
            "dec $missing : nat\n",
            "dec $same_dimension<K>(K*, K?) : bool\n",
            "def $same_dimension<K>(K_x*, K_x?) = true\n",
        ),
    )
    .unwrap();
    let (result, warnings) = elaborate::convert_with_warnings(spec_el);
    assert!(result.is_err());
    assert_eq!(warnings.len(), 1);
    let diagnostic = cause(&warnings[0]);
    assert_eq!(diagnostic.severity, Severity::Warning);
    assert_eq!(diagnostic.code.as_deref(), Some("elab/function-clause-missing"));
    assert_eq!(diagnostic.labels[0].span.left.line, 1);
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
