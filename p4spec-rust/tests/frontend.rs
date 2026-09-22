#[path = "frontend/error.rs"]
mod error;
#[path = "frontend/lexer.rs"]
mod lexer;
#[path = "frontend/parse.rs"]
mod parse;
#[path = "support/spec.rs"]
mod spec_fixture;

fn cause(report: &p4spec_rust::diagnostic::Report) -> &p4spec_rust::diagnostic::Diagnostic {
    let p4spec_rust::diagnostic::ReportKind::Cause(diagnostic) = &report.kind else {
        panic!("expected a frontend cause")
    };
    diagnostic
}
