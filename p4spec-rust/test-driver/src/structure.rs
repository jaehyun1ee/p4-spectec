use crate::{Error, Result, snapshot};
use expect_test::expect_file;
use indicatif::{ProgressBar, ProgressStyle};
use p4spec_rust::{
    frontend::parse::parse_files,
    lang::traits::print::Print,
    pass::{algo, elaborate, structure},
};
use std::{path::Path, time::Instant};

pub fn run() -> Result<()> {
    let start = Instant::now();
    let progress = ProgressBar::new(2).with_style(
        ProgressStyle::with_template("[{bar:24}] {pos}/{len} {elapsed_precise} {msg}")
            .map_err(|error| Error::Invalid(error.to_string()))?,
    );
    progress.set_message("structure: full specification");
    let spec_el =
        parse_files([Path::new("spec")]).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_il =
        elaborate::elaborate(spec_el).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_al = algo::convert(spec_il).map_err(|error| Error::Invalid(error.to_string()))?;
    let num_defs = spec_al.len();
    for (spec_al, without_rule_groups, text_mode) in [
        (spec_al.clone(), true, "without-rule-groups"),
        (spec_al, false, "with-rule-groups"),
    ] {
        progress.set_message(format!("structure: {text_mode}"));
        let spec_sl = structure::convert(spec_al, without_rule_groups)
            .map_err(|error| Error::Invalid(error.to_string()))?;
        let text_actual = Print::to_string(&spec_sl) + "\n";
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("expected/structure-{text_mode}.expected"));
        snapshot::check(expect_file![path], &text_actual);
        progress.inc(1);
    }
    progress.finish_with_message("complete");
    eprintln!(
        "structure: {num_defs} definitions checked in both modes, elapsed={:.3}s",
        start.elapsed().as_secs_f64()
    );
    Ok(())
}
