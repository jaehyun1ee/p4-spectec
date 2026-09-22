//! Elaboration declaration diagnostic acceptance
//!
//! Every fixture must parse before elaboration is exercised.
//! Error cases reject conversion, while warning cases convert successfully.
//! Structured producer, severity, code, labels, and spans are checked before
//! the shared runner renders the complete diagnostic snapshot.

use p4spec_rust::{
    diagnostic::{Diagnostic, LabelStyle, Report, ReportKind, Severity},
    frontend::parse::parse_files,
    pass::elaborate::convert_with_warnings,
};

use super::failure;
use crate::Result;

// = Expectations

#[derive(Clone, Copy)]
enum Outcome {
    Error,
    Warning,
}

#[derive(Clone, Copy)]
struct ExpectedPosition {
    line: usize,
    column: usize,
}

#[derive(Clone, Copy)]
struct ExpectedLabel {
    style: LabelStyle,
    pos_l: ExpectedPosition,
    pos_r: ExpectedPosition,
}

struct Expectation {
    outcome: Outcome,
    code: &'static str,
    labels: Vec<ExpectedLabel>,
}

const fn pos(line: usize, column: usize) -> ExpectedPosition {
    ExpectedPosition { line, column }
}

const fn primary(line_l: usize, column_l: usize, line_r: usize, column_r: usize) -> ExpectedLabel {
    ExpectedLabel {
        style: LabelStyle::Primary,
        pos_l: pos(line_l, column_l),
        pos_r: pos(line_r, column_r),
    }
}

const fn secondary(
    line_l: usize,
    column_l: usize,
    line_r: usize,
    column_r: usize,
) -> ExpectedLabel {
    ExpectedLabel {
        style: LabelStyle::Secondary,
        pos_l: pos(line_l, column_l),
        pos_r: pos(line_r, column_r),
    }
}

/// Returns the independently reviewed semantic shape for one declaration fixture.
fn case_expectation(name: &str) -> Option<Expectation> {
    let error = Outcome::Error;
    let warning = Outcome::Warning;
    Some(match name {
        "ctx-builtin-dec-redefined.watsup" => Expectation {
            outcome: error,
            code: "elab/function-builtin-repeated",
            labels: vec![primary(4, 13, 4, 14), secondary(3, 13, 3, 14)],
        },
        "ctx-builtin-dec-tparam-duplicate.watsup" => Expectation {
            outcome: error,
            code: "elab/function-builtin-type-parameter-repeated",
            labels: vec![primary(4, 21, 4, 22), secondary(4, 18, 4, 19)],
        },
        "ctx-dec-redefined.watsup" => Expectation {
            outcome: error,
            code: "elab/function-repeated",
            labels: vec![primary(4, 5, 4, 7), secondary(3, 5, 3, 7)],
        },
        "ctx-dec-tparam-duplicate.watsup" => Expectation {
            outcome: error,
            code: "elab/function-type-parameter-repeated",
            labels: vec![primary(3, 13, 3, 14), secondary(3, 10, 3, 11)],
        },
        "ctx-dec-undefined.watsup" => Expectation {
            outcome: error,
            code: "elab/function-undefined",
            labels: vec![primary(6, 18, 6, 26)],
        },
        "ctx-defined-dec-undefined.watsup" => Expectation {
            outcome: error,
            code: "elab/function-declaration-required",
            labels: vec![primary(3, 5, 3, 13)],
        },
        "ctx-extern-dec-redefined.watsup" => Expectation {
            outcome: error,
            code: "elab/function-extern-repeated",
            labels: vec![primary(2, 12, 2, 13), secondary(1, 12, 1, 13)],
        },
        "ctx-extern-dec-tparam-duplicate.watsup" => Expectation {
            outcome: error,
            code: "elab/function-extern-type-parameter-repeated",
            labels: vec![primary(1, 20, 1, 21), secondary(1, 17, 1, 18)],
        },
        "ctx-extern-relation-redefined.watsup" => Expectation {
            outcome: error,
            code: "elab/relation-extern-repeated",
            labels: vec![primary(5, 16, 5, 17), secondary(1, 16, 1, 17)],
        },
        "ctx-extern-relation-rules.watsup" => Expectation {
            outcome: error,
            code: "elab/relation-extern-rule-unsupported",
            labels: vec![primary(5, 5, 5, 6), secondary(1, 16, 1, 17)],
        },
        "ctx-function-otherwise-redefined.watsup" => Expectation {
            outcome: error,
            code: "elab/function-otherwise-repeated",
            labels: vec![primary(8, 0, 9, 14), secondary(5, 0, 6, 14)],
        },
        "ctx-metavar-id-has-suffix.watsup" => Expectation {
            outcome: error,
            code: "elab/meta-variable-identifier-invalid",
            labels: vec![primary(4, 4, 4, 7)],
        },
        "ctx-metavar-redefined.watsup" => Expectation {
            outcome: error,
            code: "elab/meta-variable-repeated",
            labels: vec![primary(4, 4, 4, 5), secondary(3, 4, 3, 5)],
        },
        "ctx-otherwise-redefined.watsup" => Expectation {
            outcome: error,
            code: "elab/relation-otherwise-repeated",
            labels: vec![primary(10, 0, 12, 14), secondary(6, 0, 8, 14)],
        },
        "ctx-relation-redefined.watsup" => Expectation {
            outcome: error,
            code: "elab/relation-repeated",
            labels: vec![primary(8, 9, 8, 10), secondary(5, 9, 5, 10)],
        },
        "ctx-relation-undefined.watsup" => Expectation {
            outcome: error,
            code: "elab/relation-rule-undefined",
            labels: vec![primary(5, 5, 5, 12)],
        },
        "ctx-rulegroup-redefined.watsup" => Expectation {
            outcome: error,
            code: "elab/relation-rule-group-repeated",
            labels: vec![primary(9, 6, 9, 11), secondary(6, 6, 6, 11)],
        },
        "ctx-table-dec-redefined.watsup" => Expectation {
            outcome: error,
            code: "elab/function-table-repeated",
            labels: vec![primary(2, 9, 2, 16), secondary(1, 9, 1, 16)],
        },
        "ctx-table-dec-undefined.watsup" => Expectation {
            outcome: error,
            code: "elab/function-table-undefined",
            labels: vec![primary(1, 9, 1, 16)],
        },
        "ctx-table-function-required.watsup" => Expectation {
            outcome: error,
            code: "elab/function-table-required",
            labels: vec![primary(3, 9, 3, 15), secondary(1, 5, 1, 12)],
        },
        "ctx-table-rows-redefined.watsup" => Expectation {
            outcome: error,
            code: "elab/table-row-repeated",
            labels: vec![primary(6, 9, 6, 15), secondary(4, 4, 4, 13)],
        },
        "ctx-type-already-defined-in-var.watsup" => Expectation {
            outcome: error,
            code: "elab/meta-variable-type-repeated",
            labels: vec![primary(6, 4, 6, 7), secondary(4, 7, 4, 10)],
        },
        "ctx-type-fully-redefined.watsup" => Expectation {
            outcome: error,
            code: "elab/type-definition-repeated",
            labels: vec![primary(4, 7, 4, 10), secondary(3, 7, 3, 10)],
        },
        "relation-missing-rules.watsup" => Expectation {
            outcome: warning,
            code: "elab/relation-rule-missing",
            labels: vec![primary(6, 0, 7, 16)],
        },
        "table-function-parameter.watsup" => Expectation {
            outcome: error,
            code: "elab/table-parameter-unsupported",
            labels: vec![primary(1, 16, 1, 30)],
        },
        "table-missing-rows.watsup" => Expectation {
            outcome: warning,
            code: "elab/table-row-missing",
            labels: vec![primary(1, 0, 1, 27)],
        },
        "table-non-bool-return.watsup" => Expectation {
            outcome: error,
            code: "elab/table-return-type-invalid",
            labels: vec![primary(7, 23, 7, 26)],
        },
        "type-dec-missing-clauses.watsup" => Expectation {
            outcome: warning,
            code: "elab/function-clause-missing",
            labels: vec![primary(1, 0, 1, 18)],
        },
        _ => return None,
    })
}

// = Validation

/// Finds the sole cause carrying the expected stable code.
fn find_diagnostic<'a>(name: &str, report: &'a Report, code: &str) -> Result<&'a Diagnostic> {
    let mut reports_pending = vec![report];
    let mut diagnostics = Vec::new();

    // Visit children in display order while retaining the full frame tree
    while let Some(report_current) = reports_pending.pop() {
        if let ReportKind::Cause(diagnostic) = &report_current.kind
            && diagnostic.code.as_deref() == Some(code)
        {
            diagnostics.push(diagnostic);
        }
        reports_pending.extend(report_current.children.iter().rev());
    }

    // Require exactly one cause to carry the case's stable identity
    if diagnostics.len() != 1 {
        return Err(failure(
            name,
            format!("expected one cause with code {code}, got {}", diagnostics.len()),
        ));
    }
    Ok(diagnostics[0])
}

/// Checks one report's semantic identity and source locations.
fn validate(name: &str, report: &Report, expectation: &Expectation) -> Result<()> {
    // Undefined calls retain the attempt frame until D04 migrates that path
    if expectation.code == "elab/function-undefined" {
        if !matches!(report.kind, ReportKind::Frame { .. }) || report.children.is_empty() {
            return Err(failure(name, "expected a nonempty elaboration attempt frame"));
        }
    } else if !matches!(report.kind, ReportKind::Cause(_)) || !report.children.is_empty() {
        return Err(failure(name, "expected a leaf diagnostic cause"));
    }

    let diagnostic = find_diagnostic(name, report, expectation.code)?;

    // Check the producer identity before inspecting its source roles
    let severity_expect = match expectation.outcome {
        Outcome::Error => Severity::Error,
        Outcome::Warning => Severity::Warning,
    };
    if diagnostic.source != "elab" {
        return Err(failure(name, format!("expected producer elab, got {}", diagnostic.source)));
    }
    if diagnostic.severity != severity_expect {
        return Err(failure(
            name,
            format!("expected severity {severity_expect:?}, got {:?}", diagnostic.severity),
        ));
    }
    if diagnostic.code.as_deref() != Some(expectation.code) {
        return Err(failure(
            name,
            format!("expected code {}, got {:?}", expectation.code, diagnostic.code),
        ));
    }
    if diagnostic.labels.len() != expectation.labels.len() {
        return Err(failure(
            name,
            format!(
                "expected {} labels, got {}",
                expectation.labels.len(),
                diagnostic.labels.len()
            ),
        ));
    }

    // Preserve both endpoints and roles instead of relying on rendered carets
    let path = format!("elab/{name}");
    for (idx, (label, label_expect)) in diagnostic
        .labels
        .iter()
        .zip(&expectation.labels)
        .enumerate()
    {
        if label.style != label_expect.style {
            return Err(failure(
                name,
                format!(
                    "label {idx}: expected style {:?}, got {:?}",
                    label_expect.style, label.style
                ),
            ));
        }
        if label.span.left.file.as_ref() != path || label.span.right.file.as_ref() != path {
            return Err(failure(
                name,
                format!("label {idx}: expected file {path}, got {}", label.span),
            ));
        }
        let pos_l = (label.span.left.line, label.span.left.column);
        let pos_r = (label.span.right.line, label.span.right.column);
        let pos_l_expect = (label_expect.pos_l.line, label_expect.pos_l.column);
        let pos_r_expect = (label_expect.pos_r.line, label_expect.pos_r.column);
        if pos_l != pos_l_expect || pos_r != pos_r_expect {
            return Err(failure(
                name,
                format!(
                    "label {idx}: expected {pos_l_expect:?}..{pos_r_expect:?}, got {pos_l:?}..{pos_r:?}"
                ),
            ));
        }
    }
    Ok(())
}

// = Entry point

/// Parses and elaborates one declaration fixture, returning its checked report.
pub fn run(name: &str) -> Result<Box<Report>> {
    let Some(expectation) = case_expectation(name) else {
        return Err(failure(name, "missing semantic diagnostic expectation"));
    };

    // Parse separately so an earlier frontend error cannot satisfy this suite
    let path = format!("elab/{name}");
    let spec_el = parse_files([&path])
        .map_err(|report| failure(name, format!("parser failed before elaboration: {report}")))?;
    let (spec_result_il, reports_warning) = convert_with_warnings(spec_el);

    // Separate rejecting errors from diagnostics returned with successful output
    let report = match expectation.outcome {
        Outcome::Error => {
            if !reports_warning.is_empty() {
                return Err(failure(
                    name,
                    format!("error case emitted {} warnings", reports_warning.len()),
                ));
            }
            spec_result_il.map_or_else(Ok, |_| {
                Err(failure(name, "elaborator unexpectedly accepted negative input"))
            })?
        }
        Outcome::Warning => {
            if let Err(report) = spec_result_il {
                return Err(failure(name, format!("warning case failed elaboration: {report}")));
            }
            if reports_warning.len() != 1 {
                return Err(failure(
                    name,
                    format!("expected one warning, got {}", reports_warning.len()),
                ));
            }
            Box::new(
                reports_warning
                    .into_iter()
                    .next()
                    .expect("checked one warning"),
            )
        }
    };

    // Verify the semantic report before snapshot rendering
    validate(name, &report, &expectation)?;
    Ok(report)
}
