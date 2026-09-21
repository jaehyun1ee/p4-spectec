use crate::{Error, Result, snapshot};
use expect_test::expect_file;
use indicatif::{ProgressBar, ProgressStyle};
use p4spec_rust::{
    frontend::parse::parse_files,
    lang::traits::print::Print,
    pass::{algo, elaborate, prosify, structure},
};
use std::{path::Path, time::Instant};

pub fn run() -> Result<()> {
    let start = Instant::now();
    let progress = ProgressBar::new(1).with_style(
        ProgressStyle::with_template("[{bar:24}] {pos}/{len} {elapsed_precise} {msg}")
            .map_err(|error| Error::Invalid(error.to_string()))?,
    );
    progress.set_message("prose: full specification");
    let spec_el =
        parse_files([Path::new("spec")]).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_il = elaborate::convert(spec_el).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_al = algo::convert(spec_il).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_sl =
        structure::convert(spec_al, false).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_pl = prosify::convert(spec_sl).map_err(|error| Error::Invalid(error.to_string()))?;
    let text_actual = Print::to_string(&spec_pl) + "\n";
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("expected/prose.expected");
    snapshot::check(expect_file![path], &text_actual);
    progress.finish_with_message("complete");

    eprintln!(
        "prose: specification snapshot checked, elapsed={:.3}s",
        start.elapsed().as_secs_f64()
    );
    Ok(())
}
