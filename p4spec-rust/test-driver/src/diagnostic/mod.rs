//! Native diagnostic snapshot acceptance
//!
//! Each case executes a product API and renders its report without colors.
//! The complete output is compared with expect-test, preserving whitespace.

mod cases;
mod parse;

use std::path::Path;

use clap::ValueEnum;
use expect_test::expect_file;
use indicatif::ProgressBar;
use p4spec_rust::diagnostic::{RenderConfig, Renderer};

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
}

// = Acceptance runner

/// Executes diagnostic inputs and compares their rendered output.
pub fn run(suite: Option<Suite>) -> Result<()> {
    let (name_suite, cases) = match suite.unwrap_or(Suite::Parse) {
        Suite::Parse => ("parse", cases::PARSE),
    };

    // Keep source identities independent of the checkout location
    std::env::set_current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/diagnostic"))?;
    eprintln!("diagnostics: OCaml reference {}", cases::REVISION);
    let progress = ProgressBar::new(cases.len() as u64);
    let mut text = String::new();

    // Execute each negative input before rendering its diagnostic
    for name in cases {
        let report = parse::run(name)?;
        let rendered = Renderer::new(RenderConfig::default())
            .render_plain(&report)
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
