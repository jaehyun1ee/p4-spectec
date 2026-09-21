//! Real SpecTec parser inputs for diagnostic acceptance
//!
//! The first case exercises the lexer through the public file parser.
//! Remaining reference cases become active with their frontend migration.

use p4spec_rust::{
    diagnostic::Report,
    frontend::{error::FrontendError, parse::parse_files},
};

use super::cases::Case;
use crate::Result;

/// Runs a source fixture through the public parser and retains its diagnostic.
pub fn run(case: &Case) -> Result<Box<Report>> {
    let path = format!("parse/{}", case.name);
    match parse_files([&path]) {
        Ok(_) => Err(case.failure("parser unexpectedly accepted negative input")),
        Err(FrontendError::Diagnostic(report)) => Ok(report),
        Err(error) => {
            Err(case.failure(format!("parser failed outside the migrated check: {error}")))
        }
    }
}
