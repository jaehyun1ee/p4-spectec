use crate::{Error, Result, progress::Progress};

// Renderers leave decorative spaces after declarations and rule-group headers
fn normalize_rendered(text: &str) -> String {
    text.split('\n')
        .map(|line| line.trim_end_matches([' ', '\t']))
        .collect::<Vec<_>>()
        .join("\n")
}
use p4spec_rust::{
    frontend::parse::parse_files,
    lang::traits::print::Print,
    pass::{algo, elaborate},
};
use std::{fs, path::Path, time::Instant};

pub fn compare(expected: &str, actual: &str, label: &str) -> Result<()> {
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
    let progress = Progress::new(2);
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
    progress.show(1, &format!("{stage}: rejected inputs"));
    let manifest = fs::read_to_string(format!(
        "p4spec-rust/test-driver/expected/{stage}-errors.expected"
    ))?;
    let mut seen = std::collections::BTreeSet::new();
    for line in manifest.lines() {
        let (name, diagnostic) = line
            .split_once('\t')
            .ok_or_else(|| Error::Invalid("invalid rejection expected record".to_owned()))?;
        if !seen.insert(name) || name.contains('/') || !name.ends_with(".watsup") {
            return Err(Error::Invalid(format!(
                "invalid or duplicate rejection input {name}"
            )));
        }
        let path = Path::new("p4spec-rust/test-driver/inputs")
            .join(stage)
            .join(name);
        let spec_el = parse_files([path]).map_err(|error| Error::Invalid(error.to_string()))?;
        let actual = if convert {
            let spec_il =
                elaborate::elaborate(spec_el).map_err(|error| Error::Invalid(error.to_string()))?;
            algo::convert(spec_il)
                .expect_err("expected conversion rejection")
                .to_string()
        } else {
            elaborate::elaborate(spec_el)
                .expect_err("expected elaboration rejection")
                .to_string()
        };
        let actual =
            serde_json::to_string(&actual).map_err(|error| Error::Invalid(error.to_string()))?;
        compare(diagnostic, &actual, name)?;
    }
    let inputs = crate::corpus::collect(
        &Path::new("p4spec-rust/test-driver/inputs").join(stage),
        ".watsup",
    )?;
    if inputs.len() != seen.len()
        || inputs
            .iter()
            .any(|path| !seen.contains(path.file_name().unwrap().to_str().unwrap()))
    {
        return Err(Error::Invalid(
            "rejection input inventory differs from expected".to_owned(),
        ));
    }
    if seen.is_empty() {
        return Err(Error::Invalid("empty rejection suite".to_owned()));
    }
    progress.show(2, "complete");
    eprintln!(
        "{stage}: full snapshot and {} rejected inputs matched, elapsed={:.3}s",
        seen.len(),
        start.elapsed().as_secs_f64()
    );
    Ok(())
}

#[cfg(test)]
mod tests;
