use crate::{
    diagnostic::{Diagnostic, Label, LabelStyle, Report, ReportKind, Severity},
    lang::{
        common::source::{Position, Span},
        il::ast::TypKind,
    },
    pass::elaborate::{
        attempt::{Backtrack, Specificity, choose_sequential, fail_silent},
        context::Context,
        error,
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

    assert_eq!(type_error.kind, TypeErrorKind::UndefinedType("Missing".to_owned()));
    assert_eq!(type_error.span, span);
    let report = error::type_operation_invalid("expand type", type_error);
    let ReportKind::Cause(diagnostic) = &report.kind else { panic!("expected type cause") };
    assert_eq!(diagnostic.code.as_deref(), Some("elab/type-operation-invalid"));
    assert_eq!(diagnostic.labels[0].span, span);
    assert!(diagnostic.message.contains("Missing"));
}

fn foreign_report() -> Box<Report> {
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
    assert_eq!(diagnostic.labels.len(), 1);
    assert_eq!(
        diagnostic.labels[0].span,
        Span::new(Position::new("foreign.watsup", 2, 1), Position::new("foreign.watsup", 2, 4))
    );
    assert_eq!(diagnostic.labels[0].message, "original label");
    assert_eq!(report.children.len(), 1);
    assert!(
        matches!(&report.children[0].kind, ReportKind::Frame { message, .. } if message == "original child")
    );
}

#[test]
fn test_attempt_preserves_a_complete_foreign_report() {
    let report = Backtrack::from(foreign_report()).into_error();
    assert_preserved(&report.children[0]);
}

fn failure(message: &str, span: Span, specificity: Specificity) -> Backtrack {
    let diagnostic = Diagnostic {
        severity: Severity::Error,
        code: None,
        message: message.to_owned(),
        labels: vec![Label { style: LabelStyle::Primary, span, message: String::new() }],
        notes: Vec::new(),
        source: "test",
    };
    Backtrack::from_report(diagnostic.into(), specificity)
}

fn assert_summary(report: &Report, span_expected: &Span, message_expected: &str) {
    let ReportKind::Frame { span, message } = &report.kind else {
        panic!("expected summary frame")
    };
    assert_eq!(span, span_expected);
    assert_eq!(message, message_expected);
}

#[test]
fn test_backtracking_preserves_nested_reports_and_alternative_order() {
    let failure_first = Backtrack::from(foreign_report()).nest(Span::default(), "outer attempt");
    let failure_second = failure("second alternative", Span::default(), Specificity::Specific);
    let report = failure_first.merge(failure_second).into_error();
    assert_eq!(report.children.len(), 2);
    assert_eq!(report.children[0].children.len(), 1);
    assert_preserved(&report.children[0].children[0]);
    let ReportKind::Cause(diagnostic) = &report.children[1].kind else { panic!("expected cause") };
    assert_eq!(diagnostic.message, "second alternative");
}

#[test]
fn test_finished_attempt_keeps_its_inner_reports_when_wrapped() {
    let report_inner =
        failure("inner failure", Span::default(), Specificity::Specific).into_error();
    let report_outer = Backtrack::from(report_inner)
        .nest(Span::default(), "outer search")
        .into_error();
    let report = &report_outer.children[0].children[0].children[0];
    let ReportKind::Cause(diagnostic) = &report.kind else {
        panic!("expected preserved inner cause")
    };
    assert_eq!(diagnostic.message, "inner failure");
}

#[test]
fn test_attempt_selection_prefers_location_then_specificity_then_depth() {
    let span =
        Span::new(Position::new("selection.watsup", 1, 0), Position::new("selection.watsup", 1, 1));
    let failure_located = failure("located generic", span.clone(), Specificity::Generic);
    let failure_specific = failure("unlocated specific", Span::default(), Specificity::Specific);
    let error = failure_located.merge(failure_specific).into_error();
    assert_summary(&error, &span, "located generic");

    let failure_specific = failure("shallow specific", span.clone(), Specificity::Specific);
    let failure_generic = failure("deep generic", span.clone(), Specificity::Generic)
        .nest(span.clone(), "generic context");
    assert_summary(
        &failure_specific.merge(failure_generic).into_error(),
        &span,
        "shallow specific",
    );

    let span_inner =
        Span::new(Position::new("selection.watsup", 2, 0), Position::new("selection.watsup", 2, 1));
    let failure_shallow = failure("shallow specific", span.clone(), Specificity::Specific);
    let failure_deep = failure("deeper specific", span_inner.clone(), Specificity::Specific)
        .nest(span, "generic context");
    assert_summary(
        &failure_shallow.merge(failure_deep).into_error(),
        &span_inner,
        "deeper specific",
    );
}

#[test]
fn test_attempt_selection_keeps_the_first_equal_ranked_failure() {
    let failure_first = failure("first", Span::default(), Specificity::Specific);
    let failure_second = failure("second", Span::default(), Specificity::Specific);
    assert_summary(&failure_first.merge(failure_second).into_error(), &Span::default(), "first");
}

#[test]
fn test_foreign_report_children_do_not_affect_attempt_depth() {
    let failure_first = failure("first", Span::default(), Specificity::Specific);
    let failure_second = Backtrack::from(foreign_report());
    let report = failure_first.merge(failure_second).into_error();
    assert_summary(&report, &Span::default(), "first");
    assert_preserved(&report.children[1]);
}

#[test]
fn test_silent_failed_alternative_rolls_back_and_commits_the_winner() {
    let id_failed = crate::phrase!(node: "failed".to_owned(), span: Span::default());
    let id_winner = crate::phrase!(node: "winner".to_owned(), span: Span::default());
    let mut ctx = Context::new();
    let value = choose_sequential(
        &mut ctx,
        |ctx| {
            ctx.frees.insert(id_failed.clone());
            fail_silent()
        },
        |ctx| {
            assert!(!ctx.frees.contains(&id_failed));
            ctx.frees.insert(id_winner.clone());
            Ok(7)
        },
    )
    .unwrap();
    assert_eq!(value, 7);
    assert!(!ctx.frees.contains(&id_failed));
    assert!(ctx.frees.contains(&id_winner));
}

#[test]
fn test_all_failed_alternatives_keep_context_and_ordered_causes() {
    let id = crate::phrase!(node: "failed".to_owned(), span: Span::default());
    let mut ctx = Context::new();
    let result: Result<(), _> = choose_sequential(
        &mut ctx,
        |ctx| {
            ctx.frees.insert(id.clone());
            Err(failure("first", Span::default(), Specificity::Specific))
        },
        |ctx| {
            assert!(!ctx.frees.contains(&id));
            ctx.frees.insert(id.clone());
            Err(failure("second", Span::default(), Specificity::Specific))
        },
    );
    assert!(!ctx.frees.contains(&id));
    let report = result.unwrap_err().into_error();
    assert_eq!(report.children.len(), 2);
    for (report, message) in report.children.iter().zip(["first", "second"]) {
        let ReportKind::Cause(diagnostic) = &report.kind else { panic!("expected cause") };
        assert_eq!(diagnostic.message, message);
    }
}
