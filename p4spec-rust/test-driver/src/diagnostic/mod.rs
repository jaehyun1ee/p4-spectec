//! Native diagnostic acceptance and pinned migration inventory
//!
//! Active cases execute product APIs and validate diagnostic identity and location
//! before comparing plain codespan output with expect-test.
//! Pending and deferred reference cases remain visible as separate counts.

mod cases;
mod parse;

use std::{collections::HashSet, path::Path};

use clap::ValueEnum;
use expect_test::expect_file;
use indicatif::ProgressBar;
use p4spec_rust::diagnostic::{ColorChoice, LabelStyle, RenderConfig, Renderer, Report};

use crate::{Error, Result};
use cases::{CASES, Case, Kind, Location, State};

// = Suites

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
/// Selects a reference acceptance family.
pub enum Suite {
    Parse,
    Elab,
    Algo,
    Prose,
    Interp,
    Boundary,
}

impl Suite {
    fn name(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Elab => "elab",
            Self::Algo => "algo",
            Self::Prose => "prose",
            Self::Interp => "interp",
            Self::Boundary => "boundary",
        }
    }
}

// = Location helpers

impl Location {
    /// Compares byte coordinates, independently of renderer display columns.
    fn matches(&self, span: &p4spec_rust::lang::common::source::Span) -> bool {
        Path::new(span.left.file.as_ref())
            .file_name()
            .and_then(|file| file.to_str())
            == Some(self.file)
            && span.left.file == span.right.file
            && (span.left.line, span.left.column) == self.start
            && self
                .end
                .is_none_or(|end| (span.right.line, span.right.column) == end)
    }
}

// = Case validation

impl Case {
    fn failure(&self, message: impl std::fmt::Display) -> Error {
        Error::Invalid(format!(
            "{} ({}; reference {} at {}): {message}\nlocation obligations: {}",
            self.name,
            self.owner,
            self.reference_code.unwrap_or("uncoded"),
            cases::REVISION,
            self.obligations,
        ))
    }

    /// Checks identity and responsible coordinates before snapshot promotion.
    fn check(&self, report: &Report) -> Result<()> {
        // Reject a different failure even when snapshot updates are enabled
        if report.code.as_deref() != self.code
            || report.severity != self.severity
            || report.source != self.source
        {
            return Err(self.failure(format!("unexpected report: {report:?}")));
        }
        // Active parser cases pin every label role before snapshot promotion
        if self.suite == Suite::Parse {
            let labels = &report.labels;
            let count_primary = usize::from(self.primary.is_some());
            if labels.len() != count_primary + self.secondary.len()
                || labels.iter().any(|label| label.message.is_empty())
                || !report.traces.is_empty()
            {
                return Err(self.failure("unexpected parser label count, message, or trace"));
            }
            let locations = self
                .primary
                .iter()
                .map(|loc| (LabelStyle::Primary, loc))
                .chain(
                    self.secondary
                        .iter()
                        .map(|loc| (LabelStyle::Secondary, loc)),
                );
            for (label, (style, loc)) in labels.iter().zip(locations) {
                if label.style != style || !loc.matches(&label.span) {
                    return Err(self.failure(format!("unexpected parser label: {label:?}")));
                }
            }
        } else if let Some(loc) = &self.primary {
            // Future suites retain their existing primary-coordinate obligation
            if !report
                .labels
                .iter()
                .any(|label| label.style == LabelStyle::Primary && loc.matches(&label.span))
            {
                return Err(self.failure("missing expected primary label"));
            }
        }
        Ok(())
    }
}

// = Acceptance runner

/// Executes active suites while reporting pending and future-gate counts.
pub fn run(suite: Option<Suite>) -> Result<()> {
    // Use fixture-relative source identities independent of checkout location
    std::env::set_current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/diagnostic"))?;
    let mut names = HashSet::new();
    for case in CASES {
        if !names.insert((case.suite.name(), case.name)) {
            return Err(case.failure("duplicate reference case"));
        }
    }
    eprintln!("diagnostics: OCaml reference {}", cases::REVISION);

    // Keep unimplemented cases distinct from actual passing acceptance
    for suite in
        [Suite::Parse, Suite::Elab, Suite::Algo, Suite::Prose, Suite::Interp, Suite::Boundary]
            .into_iter()
            .filter(|selected| suite.is_none_or(|suite| suite == *selected))
    {
        let cases: Vec<_> = CASES.iter().filter(|case| case.suite == suite).collect();
        let active = cases
            .iter()
            .filter(|case| case.state == State::Active)
            .count();
        let pending = cases
            .iter()
            .filter(|case| case.state == State::Pending)
            .count();
        let deferred = cases.len() - active - pending;
        let mut text = String::new();
        let progress = ProgressBar::new(active as u64);
        let mut outcomes = [0usize; 3];

        // Execute the real producer before rendering or comparing expected output
        for case in cases.into_iter().filter(|case| case.state == State::Active) {
            let report = match suite {
                Suite::Parse => parse::run(case)?,
                _ => return Err(case.failure("active suite has no implementation")),
            };
            case.check(&report)?;
            outcomes[match case.kind {
                Kind::Failure => 0,
                Kind::Warning => 1,
                Kind::Preservation => 2,
            }] += 1;
            let mut renderer =
                Renderer::new(RenderConfig { color: ColorChoice::Never, ..Default::default() });
            let rendered = renderer
                .render_plain(&report)
                .map_err(|error| case.failure(error))?;
            // Delimit each case without trimming codespan's trailing newlines
            text.push_str(&format!("=== {} ===\n{rendered}---\n", case.name));
            progress.inc(1);
        }
        progress.finish_and_clear();

        // Snapshot only executed cases; expect-test performs explicit promotion
        if active != 0 {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("expected/diagnostic/{}.expected", suite.name()));
            expect_file![path].assert_eq(&text);
        }
        eprintln!(
            "diagnostics/{}: {active} active passed ({} failures, {} warnings, {} preservation), {pending} unimplemented, {deferred} deferred",
            suite.name(),
            outcomes[0],
            outcomes[1],
            outcomes[2]
        );
    }
    Ok(())
}
