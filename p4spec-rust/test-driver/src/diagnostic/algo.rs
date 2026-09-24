//! Pinned algorithmic conversion failures
//!
//! Every fixture must parse and elaborate before the algorithmic check fails.
//! The runner compares the complete rendered report with its adjacent expectation.

use super::failure;
use crate::Result;
use p4spec_rust::{
    diagnostic::Report,
    frontend::parse::parse_files,
    pass::{algo, elaborate},
};

/// Runs one source fixture through its intended algorithmic failure.
pub fn run(name: &str) -> Result<Vec<Report>> {
    // Earlier stage failures are setup errors, never accepted snapshots
    let spec_el = parse_files([format!("algo/{name}")]).map_err(|report| {
        failure(name, format!("parser failed before algorithmic conversion: {report}"))
    })?;
    let spec_il = elaborate::convert(spec_el).map_err(|report| {
        failure(name, format!("elaboration failed before algorithmic conversion: {report}"))
    })?;
    // Reject unexpected success even during snapshot promotion
    let report = algo::convert(spec_il)
        .err()
        .ok_or_else(|| failure(name, "algorithmic conversion unexpectedly succeeded"))?;
    Ok(vec![*report])
}
