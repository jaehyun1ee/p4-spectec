use std::cell::Cell;

use p4spectec::{
    diagnostic::{Diagnostic, Label, Report, ReportKind, Severity},
    interp::shared::backtrack::{self, Backtrack, WithFrame},
    lang::common::source::{FileId, Position, Span},
    runner::InterpreterError,
};

/// Creates a diagnostic with optional existing source attribution.
fn diagnostic(labels: Vec<Label>) -> Diagnostic {
    Diagnostic::new("test", Severity::Error, Some("test/failure".into()), "failed", labels, vec![])
}

#[test]
fn successful_backtracking_keeps_diagnostics_and_messages_lazy() {
    let calls = Cell::new(0);
    let span = Span::default();
    let result: Backtrack<u32> = Ok(17);
    assert_eq!(
        result
            .with_frame(span, || {
                calls.set(calls.get() + 1);
                "unused".into()
            })
            .unwrap(),
        17
    );
    backtrack::check(true, span, || {
        calls.set(calls.get() + 1);
        diagnostic(vec![])
    })
    .unwrap();
    let result: Result<u32, Box<Report>> = Ok(19);
    assert_eq!(backtrack::from_result(result, &span).unwrap(), 19);
    assert_eq!(calls.get(), 0);
}

#[test]
fn outlined_failures_preserve_kinds_labels_and_ordered_frames() {
    let file = FileId::intern("backtrack-test.watsup");
    let span = Span::new(Position::new(file, 3, 1), Position::new(file, 3, 2));
    let span_existing = Span::new(Position::new(file, 7, 1), Position::new(file, 7, 2));
    for existing in [false, true] {
        let labels =
            if existing { vec![Label::primary(&span_existing, "original")] } else { vec![] };
        let result: Result<(), Box<Report>> = Err(Box::new(diagnostic(labels).into()));
        let error = backtrack::from_result(result, &span)
            .with_frame(span, || "outer".into())
            .unwrap_err();
        let InterpreterError::Fatal(report) = error else {
            panic!("fatal kind must survive");
        };
        assert!(
            matches!(&report.kind, ReportKind::Frame { span: span_frame, message } if *span_frame==span && message=="outer")
        );
        let ReportKind::Cause(diagnostic) = &report.children[0].kind else {
            panic!("cause must survive");
        };
        assert_eq!(diagnostic.labels.len(), 1);
        assert_eq!(diagnostic.labels[0].span, if existing { span_existing } else { span });
    }
    let result: Backtrack<()> = Err(InterpreterError::Mismatch(vec![
        Report::frame(span_existing, "first", vec![]),
        Report::frame(span, "second", vec![]),
    ]));
    let error = result.with_frame(span, || "outer".into()).unwrap_err();
    let InterpreterError::Mismatch(reports) = error else {
        panic!("mismatch kind must survive");
    };
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].children.len(), 2);
    assert_eq!(reports[0].children[0].to_string(), "note: first");
    assert_eq!(reports[0].children[1].to_string(), "note: second");
    let error = backtrack::check(false, span, || diagnostic(vec![])).unwrap_err();
    let InterpreterError::Fatal(report) = error else {
        panic!("check failure must be fatal");
    };
    let ReportKind::Cause(diagnostic) = &report.kind else {
        panic!("check must produce a cause");
    };
    assert_eq!(diagnostic.labels[0].span, span);
}
