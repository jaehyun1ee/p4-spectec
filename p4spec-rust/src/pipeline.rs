//! File-based entry points for specification transformations
//!
//! `parse`, `elab`, `algo`, `structure`, and `prosify` run the passes
//! from ordered source paths to the requested language.
//! Errors preserve the failing stage and its source diagnostics;
//! callers choose how to report them.

use std::path::Path;

use crate::{
    frontend::{error::FrontendError, parse::parse_files},
    lang::{al, el, il, pl, sl},
    pass::{
        self, algo::AlgoError, elaborate::ElabError, prosify::ProseError, structure::StructureError,
    },
};

// = Errors

/// A failure while reading or transforming a specification.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Reading or parsing source failed.
    #[error(transparent)]
    Frontend(#[from] FrontendError),
    /// Elaborating EL into IL failed.
    #[error(transparent)]
    Elab(ElabError),
    /// Converting IL into AL failed.
    #[error(transparent)]
    Algo(#[from] AlgoError),
    /// Structuring AL into SL failed.
    #[error(transparent)]
    Structure(#[from] StructureError),
    /// Converting SL into PL failed.
    #[error(transparent)]
    Prose(#[from] ProseError),
}

// = Transformations

/// Parses specification paths in processing order into EL.
pub fn parse<I, P>(paths: I) -> Result<el::ast::Spec, Error>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    Ok(parse_files(paths)?)
}

/// Parses and elaborates specification paths into typed IL.
pub fn elab<I, P>(paths: I) -> Result<il::ast::Spec, Error>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let spec_el = parse(paths)?;
    pass::elaborate::convert(spec_el).map_err(Error::Elab)
}

/// Parses, elaborates, and converts specification paths into AL.
pub fn algo<I, P>(paths: I) -> Result<al::ast::Spec, Error>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let spec_il = elab(paths)?;
    Ok(pass::algo::convert(spec_il)?)
}

/// Converts specification paths into SL, optionally removing rule groups.
///
/// Set `without_rule_groups` to true for SL execution;
/// PL conversion requires the groups to remain.
pub fn structure<I, P>(paths: I, without_rule_groups: bool) -> Result<sl::ast::Spec, Error>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let spec_al = algo(paths)?;
    Ok(pass::structure::convert(spec_al, without_rule_groups)?)
}

/// Converts specification paths through SL with rule groups into annotated PL.
pub fn prosify<I, P>(paths: I) -> Result<pl::ast::Spec, Error>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let spec_sl = structure(paths, false)?;
    Ok(pass::prosify::convert(spec_sl)?)
}
