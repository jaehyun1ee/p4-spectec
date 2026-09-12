use crate::{Error, Result, snapshot};
use expect_test::expect_file;
use indicatif::{ProgressBar, ProgressStyle};
use p4spec_rust::{
    frontend::parse::parse_files,
    lang::traits::print::Print,
    pass::{algo, elaborate},
};
use std::{path::Path, time::Instant};

pub fn run() -> Result<()> {
    let start = Instant::now();
    let progress = ProgressBar::new(1).with_style(
        ProgressStyle::with_template("[{bar:24}] {pos}/{len} {elapsed_precise} {msg}")
            .map_err(|error| Error::Invalid(error.to_string()))?,
    );
    progress.set_message("algo: full specification");
    let spec_el =
        parse_files([Path::new("spec")]).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_il =
        elaborate::elaborate(spec_el).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_al = algo::convert(spec_il).map_err(|error| Error::Invalid(error.to_string()))?;
    let actual = Print::to_string(&spec_al) + "\n";
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("expected/algo.expected");
    snapshot::check(expect_file![path], &actual);
    progress.finish_with_message("complete");
    eprintln!(
        "algo: specification snapshot checked, elapsed={:.3}s",
        start.elapsed().as_secs_f64()
    );
    Ok(())
}
