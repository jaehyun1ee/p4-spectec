use super::{RenderConfig, Renderer};
use crate::{
    diagnostic::{Diagnostic, Label, Report, Severity},
    lang::common::source::{Position, Span},
};
use codespan_reporting::term::termcolor::Buffer;

#[test]
fn colored_tree_preserves_plain_layout_and_neutral_connections() {
    let span = Span::new(Position::new("source", 1, 0), Position::new("source", 1, 3));
    let report_inner = Report::from(Diagnostic {
        severity: Severity::Error,
        code: Some("test/invalid".to_owned()),
        message: "invalid input".to_owned(),
        labels: vec![Label::primary(&span, "bad value")],
        notes: vec!["try another value".to_owned()],
        source: "test",
    });
    let report = Report::frame(
        Span::default(),
        "root",
        vec![report_inner, Report::frame(Span::default(), "last", vec![])],
    );
    let mut renderer = Renderer::new(RenderConfig::default());
    renderer.insert_source("source", "bad");
    let text_plain = renderer.render_to_string(&report).unwrap();
    let mut buffer = Buffer::ansi();
    renderer.render_to_buffer(&mut buffer, &report).unwrap();
    let text_colored = String::from_utf8(buffer.into_inner()).unwrap();
    // Connections reset the header or label style before drawing their lines
    assert!(text_colored.contains("\x1b[0m├─ "), "{text_colored:?}");
    assert!(text_colored.contains("\x1b[0m│"), "{text_colored:?}");
    assert!(text_colored.contains("\x1b[0m└─ "), "{text_colored:?}");
    // Removing SGR sequences leaves exactly the same physical tree layout
    let mut text_uncolored = String::new();
    let mut in_color = false;
    for ch in text_colored.chars() {
        if ch == '\x1b' {
            in_color = true;
        } else if in_color {
            if ch == 'm' {
                in_color = false;
            }
        } else {
            text_uncolored.push(ch);
        }
    }
    assert!(!in_color);
    assert_eq!(text_uncolored, text_plain);
}
