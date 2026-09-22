use super::{report as report_cause, span};
use p4spec_rust::{
    diagnostic::{Diagnostic, RenderConfig, Renderer, Report, ReportKind, Severity},
    lang::common::source::Span,
};

fn frame(message: &str, children: Vec<Report>) -> Report {
    Report {
        kind: ReportKind::Frame { span: Span::default(), message: message.to_owned() },
        children,
    }
}

fn failure(message: &str, children: Vec<Report>) -> Report {
    Report {
        kind: ReportKind::Cause(Diagnostic {
            severity: Severity::Error,
            code: Some("test/failure".to_owned()),
            message: message.to_owned(),
            labels: vec![],
            notes: vec![],
            source: "test",
        }),
        children,
    }
}

#[test]
fn summary_does_not_flatten_trace_diagnostics() {
    let mut report = report_cause(span("absent.watsup", 2, 1, 2, 3));
    report.children.push(super::report(Span::default()));
    let summary = report.to_string();
    assert!(summary.contains("parse/text-escape-invalid"));
    assert!(summary.contains("invalid escape in text literal"));
    assert!(!summary.contains("use a supported escape"));
    let ReportKind::Cause(cause) = &report.children[0].kind else { panic!("diagnostic preserved") };
    assert_eq!(cause.source, "parse");
    assert_eq!(cause.notes, ["use a supported escape"]);
}

#[test]
fn deep_mixed_traces_render_and_drop_on_a_small_stack() {
    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            let mut trace = frame("leaf", vec![]);
            for idx in 0..20_000 {
                trace = if idx % 2 == 0 {
                    let mut cause = report_cause(Span::default());
                    cause.children.push(trace);
                    cause
                } else {
                    frame(&format!("frame {idx}"), vec![trace])
                };
            }
            let mut report = report_cause(Span::default());
            report.children.push(trace);
            let mut renderer =
                Renderer::new(RenderConfig { trace_limit: 20_001, ..Default::default() });
            let text = renderer.render_to_string(&report).unwrap();
            assert!(text.contains("leaf"));
            assert!(format!("{report:?}").len() < 1_000);
            drop(report);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn trace_limit_preserves_branches_and_the_underlying_reports() {
    let mut report = report_cause(Span::default());
    for message in ["first branch", "second branch"] {
        report.children.push(frame(message, vec![]));
    }
    let mut renderer = Renderer::new(RenderConfig { trace_limit: 1, ..Default::default() });
    let text = renderer.render_to_string(&report).unwrap();
    assert!(text.contains("first branch"));
    assert!(!text.contains("second branch"));
    assert!(text.contains("truncated"));
    assert_eq!(report.children.len(), 2);
    let text = Renderer::new(RenderConfig::default())
        .render_to_string(&report)
        .unwrap();
    assert!(text.find("first branch").unwrap() < text.find("second branch").unwrap());
}

#[test]
fn root_frames_and_mixed_branches_preserve_depth_and_order() {
    let report = frame(
        "execution",
        vec![
            frame("call", vec![failure("argument", vec![failure("type", vec![])])]),
            failure("alternative", vec![]),
        ],
    );
    let text = Renderer::new(RenderConfig::default())
        .render_to_string(&report)
        .unwrap();
    assert_eq!(
        text,
        concat!(
            "note: execution\n\n",
            "trace[0]:\nnote: call\n\n",
            "trace[1]:\nerror[test/failure]: argument\n\n",
            "trace[2]:\nerror[test/failure]: type\n\n",
            "trace[0]:\nerror[test/failure]: alternative\n\n",
        )
    );
    assert_eq!(report.to_string(), "note: execution");
}

#[test]
fn cause_roots_preserve_nested_metadata_and_source_snippets() {
    let mut report = report_cause(span("root", 1, 0, 1, 4));
    let mut cause_inner = super::report(span("cause", 1, 0, 1, 5));
    let ReportKind::Cause(diagnostic) = &mut cause_inner.kind else { unreachable!() };
    diagnostic.severity = Severity::Warning;
    diagnostic.code = None;
    diagnostic.source = "nested";
    let frame_inner = Report {
        kind: ReportKind::Frame {
            span: span("frame", 1, 0, 1, 5),
            message: "trying candidate".to_owned(),
        },
        children: vec![cause_inner],
    };
    report.children = vec![frame_inner, frame("next candidate", vec![])];
    let mut renderer = Renderer::new(RenderConfig::default());
    renderer.insert_source("root", "root");
    renderer.insert_source("frame", "frame");
    renderer.insert_source("cause", "cause");
    let text = renderer.render_to_string(&report).unwrap();
    let sections = text.split("trace[").collect::<Vec<_>>();
    assert_eq!(sections.len(), 4, "{text}");
    for (section, heading, loc, source, underline) in [
        (
            sections[0],
            "error[parse/text-escape-invalid]",
            "root:1:1",
            "1 │ root",
            "^^^^ invalid escape",
        ),
        (sections[1], "0]:\nnote: trying candidate", "frame:1:1", "1 │ frame", "-----"),
        (
            sections[2],
            "1]:\nwarning: invalid escape",
            "cause:1:1",
            "1 │ cause",
            "^^^^^ invalid escape",
        ),
    ] {
        for part in [heading, loc, source, underline] {
            assert!(section.contains(part), "missing {part:?}: {text}");
        }
    }
    assert!(sections[2].contains("= use a supported escape"), "{text}");
    assert!(sections[2].contains("= source: nested"), "{text}");
    assert_eq!(sections[3], "0]:\nnote: next candidate\n\n");

    // The same frame retains its snippet when promoted to the root
    let text = renderer.render_to_string(&report.children[0]).unwrap();
    assert!(text.starts_with("note: trying candidate"), "{text}");
    assert!(text.contains("frame:1:1"), "{text}");
    assert!(text.contains("1 │ frame"), "{text}");
    assert!(text.contains("trace[0]:\nwarning:"), "{text}");
}

#[test]
fn zero_trace_budget_keeps_either_root_kind_and_all_stored_children() {
    for report in [
        frame("root", vec![failure("child", vec![])]),
        failure("root", vec![frame("child", vec![])]),
    ] {
        let text = Renderer::new(RenderConfig { trace_limit: 0, ..Default::default() })
            .render_to_string(&report)
            .unwrap();
        assert!(text.starts_with(&report.to_string()), "{text}");
        assert!(text.contains("trace truncated after 0 nodes"), "{text}");
        assert!(!text.contains("child"), "{text}");
        assert_eq!(report.children.len(), 1);
    }
    let report = frame("root", vec![]);
    let text = Renderer::new(RenderConfig { trace_limit: 0, ..Default::default() })
        .render_to_string(&report)
        .unwrap();
    assert_eq!(text, "note: root\n\n");
}
