use crate::{Error, Result};
use p4spec_rust::{
    frontend::parse::parse_files,
    pass::{algo, elaborate, prose, structure},
};
use std::{path::Path, time::Instant};

pub fn run() -> Result<()> {
    let start = Instant::now();
    let spec_el =
        parse_files([Path::new("spec")]).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_il = elaborate::convert(spec_el).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_al = algo::convert(spec_il).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_sl =
        structure::convert(spec_al, false).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_pl = prose::prosify(spec_sl).map_err(|error| Error::Invalid(error.to_string()))?;

    eprintln!(
        "prose: {} definitions converted, elapsed={:.3}s",
        spec_pl.len(),
        start.elapsed().as_secs_f64()
    );
    Ok(())
}
