use crate::{Error, Result, progress::Progress};

use p4spec_rust::{
    frontend::parse::parse_files,
    lang::traits::print::Print,
    pass::{algo, elaborate},
};
use std::{fs, path::Path, time::Instant};

// Renderers leave decorative spaces after declarations and rule-group headers
fn normalize_rendered(text: &str) -> String {
    text.split('\n')
        .map(|line| line.trim_end_matches([' ', '\t']))
        .collect::<Vec<_>>()
        .join("\n")
}

fn compare(expected: &str, actual: &str, label: &str) -> Result<()> {
    if expected == actual {
        return Ok(());
    }
    let mut lines_expected = expected.lines();
    let mut lines_actual = actual.lines();
    for idx in 1.. {
        let line_expected = lines_expected.next();
        let line_actual = lines_actual.next();
        if line_expected != line_actual || line_expected.is_none() {
            return Err(Error::Invalid(format!(
                "{label}: mismatch at line {idx}\nexpected: {line_expected:?}\nactual: {line_actual:?}"
            )));
        }
    }
    unreachable!()
}

pub fn run(convert: bool) -> Result<()> {
    let start = Instant::now();
    let stage = if convert { "algo" } else { "elab" };
    let progress = Progress::new(1);
    progress.show(0, &format!("{stage}: full specification"));
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
    let path = format!("p4spec-rust/test-driver/expected/{stage}.expected");
    compare(
        &fs::read_to_string(&path)?,
        &normalize_rendered(&actual),
        &path,
    )?;
    progress.show(1, "complete");
    eprintln!(
        "{stage}: specification snapshot matched, elapsed={:.3}s",
        start.elapsed().as_secs_f64()
    );
    Ok(())
}
