use crate::{Error, Result};
use expect_test::expect_file;
use indicatif::{ProgressBar, ProgressStyle};

use p4spec_rust::{
    frontend::parse::parse_files,
    lang::traits::print::Print,
    pass::{algo, elaborate},
};
use std::{path::Path, time::Instant};

// Renderers leave decorative spaces after declarations and rule-group headers
fn normalize_rendered(text: &str) -> String {
    text.split('\n')
        .map(|line| line.trim_end_matches([' ', '\t']))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn run(convert: bool) -> Result<()> {
    let start = Instant::now();
    let stage = if convert { "algo" } else { "elab" };
    let progress = ProgressBar::new(1).with_style(
        ProgressStyle::with_template("[{bar:24}] {pos}/{len} {elapsed_precise} {msg}")
            .map_err(|error| Error::Invalid(error.to_string()))?,
    );
    progress.set_message(format!("{stage}: full specification"));
    let spec_el =
        parse_files([Path::new("spec")]).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_il =
        elaborate::elaborate(spec_el).map_err(|error| Error::Invalid(error.to_string()))?;
    let actual = if convert {
        let spec_al = algo::convert(spec_il).map_err(|error| Error::Invalid(error.to_string()))?;
        Print::to_string(&spec_al)
    } else {
        Print::to_string(&spec_il)
    } + "\n";
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("expected")
        .join(format!("{stage}.expected"));
    expect_file![path].assert_eq(&normalize_rendered(&actual));
    progress.finish_with_message("complete");
    eprintln!(
        "{stage}: specification snapshot checked, elapsed={:.3}s",
        start.elapsed().as_secs_f64()
    );
    Ok(())
}
