//! Native diagnostic snapshot acceptance
//!
//! Each case executes a product API and renders its report without colors.
//! The complete output is compared with expect-test, preserving whitespace.

mod cases;
mod elab;
mod parse;

use std::path::Path;

use clap::ValueEnum;
use expect_test::expect_file;
use indicatif::ProgressBar;
use p4spec_rust::diagnostic::{RenderConfig, Renderer, Report};

use crate::{Error, Result};

// = Helpers

fn failure(name: &str, message: impl std::fmt::Display) -> Error {
    Error::Invalid(format!("{name}: {message}"))
}

// = Suites

/// Selects an implemented diagnostic snapshot suite.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Suite {
    Parse,
    Elab,
}

// = Acceptance runner

/// Executes one diagnostic suite and compares its rendered output.
fn run_suite(
    name_suite: &str,
    cases: &[&str],
    run_case: fn(&str) -> Result<Box<Report>>,
) -> Result<()> {
    let progress = ProgressBar::new(cases.len() as u64);
    let mut text = String::new();

    // Execute each input and validate its semantic report before rendering
    for name in cases {
        let report = run_case(name)?;
        let rendered = Renderer::new(RenderConfig::default())
            .render_to_string(&report)
            .map_err(|error| failure(name, error))?;
        text.push_str(&format!("=== {name} ===\n{rendered}---\n"));
        progress.inc(1);
    }
    progress.finish_and_clear();

    // Compare the complete output without trimming codespan whitespace
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("expected/diagnostic/{name_suite}.expected"));
    expect_file![path].assert_eq(&text);
    eprintln!("diagnostics/{name_suite}: {} cases passed", cases.len());
    Ok(())
}

/// Executes selected diagnostic inputs and compares their rendered output.
pub fn run(suite: Option<Suite>) -> Result<()> {
    // Keep source identities independent of the checkout location
    std::env::set_current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/diagnostic"))?;
    eprintln!("diagnostics: OCaml reference {}", cases::REVISION);

    // Absence selects every active suite in stage order
    match suite {
        Some(Suite::Parse) => run_suite("parse", cases::PARSE, parse::run),
        Some(Suite::Elab) => run_elab(),
        None => {
            run_suite("parse", cases::PARSE, parse::run)?;
            run_elab()
        }
    }
}

/// Runs declaration acceptance and reports later-unit inventory separately.
fn run_elab() -> Result<()> {
    run_suite("elab", cases::ELAB, elab::run)?;
    eprintln!(
        "diagnostics/elab pending: D04 {} cases; D05 {} cases",
        cases::ELAB_D04_PENDING,
        cases::ELAB_D05_PENDING
    );
    Ok(())
}
