use crate::{Error, Result, snapshot};
use expect_test::expect_file;
use p4spec_rust::{
    frontend::parse::parse_files,
    lang::traits::print::Print,
    pass::{algo, elaborate, prose, structure},
};
use sha2::{Digest, Sha256};
use std::{path::Path, time::Instant};

fn hash(bytes: &[u8]) -> String {
    format!("{:x}\n", Sha256::digest(bytes))
}

fn normalize_rendered(text: &str) -> String {
    text.split('\n')
        .map(|line| line.trim_end_matches([' ', '\t']))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn run() -> Result<()> {
    let start = Instant::now();
    let spec_el =
        parse_files([Path::new("spec")]).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_il = elaborate::convert(spec_el).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_al = algo::convert(spec_il).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_sl =
        structure::convert(spec_al, false).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_pl = prose::convert(spec_sl).map_err(|error| Error::Invalid(error.to_string()))?;
    let rendered = normalize_rendered(&(Print::to_string(&spec_pl) + "\n"));
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("expected/prose.expected");
    snapshot::check(expect_file![path], &hash(rendered.as_bytes()));

    eprintln!(
        "prose: {} definitions checked, elapsed={:.3}s",
        spec_pl.len(),
        start.elapsed().as_secs_f64()
    );
    Ok(())
}
