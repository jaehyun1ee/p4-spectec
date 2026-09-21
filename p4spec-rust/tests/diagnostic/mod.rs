mod render;
mod report;

use p4spec_rust::{
    diagnostic::{Label, LabelStyle, Report, Severity},
    lang::common::source::{Position, Span},
};

fn span(file: &str, line_l: usize, col_l: usize, line_r: usize, col_r: usize) -> Span {
    Span::new(Position::new(file, line_l, col_l), Position::new(file, line_r, col_r))
}

fn report(span: Span) -> Report {
    Report {
        severity: Severity::Error,
        code: Some("parse/text-escape-invalid".to_owned()),
        message: "invalid escape in text literal".to_owned(),
        labels: vec![Label {
            style: LabelStyle::Primary,
            span,
            message: "invalid escape".to_owned(),
        }],
        notes: vec!["use a supported escape".to_owned()],
        source: "parse",
        traces: Vec::new(),
    }
}

#[test]
fn parser_pilot_reaches_cli_with_its_code_and_escape_span() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test-driver/fixtures/diagnostic/parse/parse-illegal-escape.watsup");
    for command in ["elab", "algo", "struct"] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_p4spec-rust"))
            .arg(command)
            .arg(&path)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let text = String::from_utf8(output.stderr).unwrap();
        assert!(text.contains("error[parse/text-escape-invalid]"), "{text}");
        assert!(text.contains("parse-illegal-escape.watsup:3:5"), "{text}");
        assert!(text.contains("^^"), "{text}");
        assert!(!text.contains('\u{1b}'), "{text}");
    }
}
