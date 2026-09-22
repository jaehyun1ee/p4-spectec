use super::{report, span};
use p4spec_rust::{
    diagnostic::{RenderConfig, Renderer, Trace},
    lang::common::source::Span,
};

#[test]
fn summary_does_not_flatten_trace_diagnostics() {
    let mut report = report(span("absent.watsup", 2, 1, 2, 3));
    report
        .traces
        .push(Trace::Cause(Box::new(super::report(Span::default()))));
    let summary = report.to_string();
    assert!(summary.contains("parse/text-escape-invalid"));
    assert!(summary.contains("invalid escape in text literal"));
    assert!(!summary.contains("use a supported escape"));
    let Trace::Cause(cause) = &report.traces[0] else { panic!("diagnostic preserved") };
    assert_eq!(cause.source, "parse");
    assert_eq!(cause.notes, ["use a supported escape"]);
}

#[test]
fn deep_mixed_traces_render_and_drop_on_a_small_stack() {
    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            let mut trace = Trace::Frame {
                span: Span::default(),
                message: "leaf".to_owned(),
                children: vec![],
            };
            for idx in 0..20_000 {
                trace = if idx % 2 == 0 {
                    let mut cause = report(Span::default());
                    cause.traces.push(trace);
                    Trace::Cause(Box::new(cause))
                } else {
                    Trace::Frame {
                        span: Span::default(),
                        message: format!("frame {idx}"),
                        children: vec![trace],
                    }
                };
            }
            let mut report = report(Span::default());
            report.traces.push(trace);
            let mut renderer =
                Renderer::new(RenderConfig { trace_limit: 20_001, ..Default::default() });
            let text = renderer.render_plain(&report).unwrap();
            assert!(text.contains("leaf"));
            drop(report);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn trace_limit_preserves_branches_and_the_underlying_reports() {
    let mut report = report(Span::default());
    for message in ["first branch", "second branch"] {
        report.traces.push(Trace::Frame {
            span: Span::default(),
            message: message.to_owned(),
            children: vec![],
        });
    }
    let mut renderer = Renderer::new(RenderConfig { trace_limit: 1, ..Default::default() });
    let text = renderer.render_plain(&report).unwrap();
    assert!(text.contains("first branch"));
    assert!(!text.contains("second branch"));
    assert!(text.contains("truncated"));
    assert_eq!(report.traces.len(), 2);
    let text = Renderer::new(RenderConfig::default())
        .render_plain(&report)
        .unwrap();
    assert!(text.find("first branch").unwrap() < text.find("second branch").unwrap());
}
