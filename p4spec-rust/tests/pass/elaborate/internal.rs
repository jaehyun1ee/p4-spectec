use crate::{
    diagnostic::{Diagnostic, Label, LabelStyle, Report, ReportKind, Severity},
    lang::{
        common::source::{Position, Span},
        il::ast::TypKind,
    },
    pass::elaborate::{
        attempt::Backtrack,
        error::{ElabErrorKind, MigrationError},
    },
    runtime::{
        envs::elab::TDEnv,
        ops::typ::{TypeErrorKind, expand_typ},
    },
};

#[test]
fn test_runtime_type_failure_keeps_its_category_and_source_span() {
    let span = Span::new(
        Position::new("elaboration.watsup", 7, 2),
        Position::new("elaboration.watsup", 7, 8),
    );
    let id = crate::phrase! {
        node: "Missing".to_owned(),
        span: span.clone(),
    };
    let typ = crate::phrase! {
        node: TypKind::Var(id, vec![]),
        span: span.clone(),
    };
    let type_error = expand_typ(&TDEnv::new(), &typ).unwrap_err();

    let error = MigrationError::from(type_error);

    assert_eq!(error.kind, ElabErrorKind::Type(TypeErrorKind::UndefinedType("Missing".to_owned())));
    assert_eq!(error.span, span);
}

fn report() -> Box<Report> {
    let span =
        Span::new(Position::new("foreign.watsup", 2, 1), Position::new("foreign.watsup", 2, 4));
    let diagnostic = Diagnostic {
        severity: Severity::Warning,
        code: Some("foreign/specific".to_owned()),
        message: "original cause".to_owned(),
        labels: vec![Label {
            style: LabelStyle::Secondary,
            span,
            message: "original label".to_owned(),
        }],
        notes: vec!["original note".to_owned()],
        source: "foreign",
    };
    let mut report = Report::from(diagnostic);
    report.children.push(Report {
        kind: ReportKind::Frame { span: Span::default(), message: "original child".to_owned() },
        children: Vec::new(),
    });
    Box::new(report)
}

fn assert_preserved(report: &Report) {
    let ReportKind::Cause(diagnostic) = &report.kind else { panic!("expected cause") };
    assert_eq!(diagnostic.source, "foreign");
    assert_eq!(diagnostic.code.as_deref(), Some("foreign/specific"));
    assert_eq!(diagnostic.severity, Severity::Warning);
    assert_eq!(diagnostic.message, "original cause");
    assert_eq!(diagnostic.notes, ["original note"]);
    assert_eq!(diagnostic.labels[0].style, LabelStyle::Secondary);
    assert_eq!(diagnostic.labels[0].span.left.line, 2);
    assert_eq!(diagnostic.labels[0].span.left.column, 1);
    assert_eq!(diagnostic.labels[0].span.right.column, 4);
    assert_eq!(diagnostic.labels[0].message, "original label");
    assert_eq!(report.children.len(), 1);
    assert!(
        matches!(&report.children[0].kind, ReportKind::Frame { message, .. } if message == "original child")
    );
}

#[test]
fn test_bridge_preserves_a_complete_foreign_report() {
    let report = MigrationError::from(report()).into_report();
    assert_preserved(&report);
}

#[test]
fn test_backtracking_preserves_nested_reports_and_alternative_order() {
    let failure = Backtrack::from(MigrationError::from(report()));
    let failure = failure.nest(MigrationError::new(
        ElabErrorKind::NoMatchingAlternative,
        Span::default(),
        "outer attempt",
    ));
    let failure = failure.merge(Backtrack::from(MigrationError::new(
        ElabErrorKind::InvalidArgument,
        Span::default(),
        "second alternative",
    )));
    let report = failure.into_error().into_report();
    assert_eq!(report.children.len(), 2);
    assert_eq!(report.children[0].children.len(), 1);
    assert_preserved(&report.children[0].children[0]);
    let ReportKind::Cause(diagnostic) = &report.children[1].kind else { panic!("expected cause") };
    assert_eq!(diagnostic.message, "second alternative");
}

#[test]
fn test_finished_legacy_attempt_keeps_its_inner_traces_when_wrapped() {
    let inner = Backtrack::from(MigrationError::new(
        ElabErrorKind::InvalidArgument,
        Span::default(),
        "inner failure",
    ))
    .into_error();
    let outer = Backtrack::from(inner)
        .nest(MigrationError::new(
            ElabErrorKind::NoMatchingAlternative,
            Span::default(),
            "outer search",
        ))
        .into_error()
        .into_report();
    let report = &outer.children[0].children[0].children[0];
    let ReportKind::Cause(diagnostic) = &report.kind else {
        panic!("expected preserved inner cause")
    };
    assert_eq!(diagnostic.message, "inner failure");
}

#[test]
fn test_attempt_selection_prefers_location_then_specificity_then_depth() {
    let span =
        Span::new(Position::new("selection.watsup", 1, 0), Position::new("selection.watsup", 1, 1));
    let located = Backtrack::from(MigrationError::new(
        ElabErrorKind::NoMatchingAlternative,
        span.clone(),
        "located generic",
    ));
    let specific = Backtrack::from(MigrationError::new(
        ElabErrorKind::InvalidArgument,
        Span::default(),
        "unlocated specific",
    ));
    let error = located.merge(specific).into_error();
    assert_eq!(error.span, span);
    assert_eq!(error.kind, ElabErrorKind::NoMatchingAlternative);

    let failure = Backtrack::from(MigrationError::new(
        ElabErrorKind::InvalidArgument,
        span.clone(),
        "located specific",
    ))
    .nest(MigrationError::new(
        ElabErrorKind::NoMatchingAlternative,
        span.clone(),
        "located generic",
    ));
    assert_eq!(failure.into_error().kind, ElabErrorKind::InvalidArgument);

    let span_inner =
        Span::new(Position::new("selection.watsup", 2, 0), Position::new("selection.watsup", 2, 1));
    let failure = Backtrack::from(MigrationError::new(
        ElabErrorKind::InvalidArgument,
        span_inner.clone(),
        "deeper specific",
    ))
    .nest(MigrationError::new(ElabErrorKind::TypeMismatch, span, "shallower specific"));
    let error = failure.into_error();
    assert_eq!(error.kind, ElabErrorKind::InvalidArgument);
    assert_eq!(error.span, span_inner);
}
