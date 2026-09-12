use crate::{
    Error, Result,
    corpus::{self, Outcome, Results},
};
use indicatif::{ProgressBar, ProgressStyle};
use p4spec_rust::{
    frontend::parse::parse_files,
    interface::p4::{
        error::P4ErrorKind,
        parse::{parse_file, parse_string},
        unparse::P4Unparser,
    },
    lang::data::value::ValueArena,
    pass::{algo, elaborate},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

fn roundtrip(
    unparser: &P4Unparser,
    includes: &[PathBuf],
    path: &Path,
) -> Result<(Outcome, Option<String>)> {
    let mut arena = ValueArena::new();
    fs::File::open(path)?;
    let program = match parse_file(&mut arena, includes, path) {
        Ok(program) => program,
        Err(error) => match error.kind {
            P4ErrorKind::Lex(_) | P4ErrorKind::Syntax => {
                return Ok((Outcome::ParseFail, Some(error.to_string())));
            }
            _ => return Err(Error::Invalid(format!("{}: {error}", path.display()))),
        },
    };
    let text = unparser
        .render(&arena, &program)
        .map_err(|error| Error::Invalid(format!("{}: {error}", path.display())))?;
    let program_roundtrip = match parse_string(&mut arena, path, &text) {
        Ok(program) => program,
        Err(error) => match error.kind {
            P4ErrorKind::Lex(_) | P4ErrorKind::Syntax => {
                let column = error.span.left.column.max(0) as usize;
                let line = text
                    .lines()
                    .nth(error.span.left.line.saturating_sub(1) as usize)
                    .unwrap_or("");
                let excerpt: String = line
                    .chars()
                    .skip(column.saturating_sub(80))
                    .take(160)
                    .collect();
                return Ok((
                    Outcome::ReparseFail,
                    Some(format!("{error}; near {excerpt:?}")),
                ));
            }
            _ => return Err(Error::Invalid(format!("{}: {error}", path.display()))),
        },
    };
    // Canonical bodies ignore source spans and type notes, like IL value equality
    let outcome = if arena.canon_id(&program) == arena.canon_id(&program_roundtrip) {
        Outcome::Pass
    } else {
        Outcome::RoundtripFail
    };
    Ok((outcome, None))
}

pub fn run() -> Result<()> {
    let start = Instant::now();
    let mut expected = BTreeMap::new();
    let mut paths = Vec::new();
    for (name, dirs) in [
        (
            "p4parse-pos.expected",
            &["p4c/testdata/p4_16_samples", "testdata/custom"][..],
        ),
        ("p4parse-neg.expected", &["p4c/testdata/p4_16_errors"][..]),
    ] {
        let text = fs::read_to_string(Path::new("p4spec-rust/test-driver/expected").join(name))?;
        let records = corpus::parse_expected(&text)?;
        for dir in dirs {
            paths.extend(corpus::collect(Path::new(dir), ".p4")?);
        }
        for (path, outcome) in records {
            if !matches!(
                outcome,
                Outcome::Pass | Outcome::ParseFail | Outcome::ReparseFail | Outcome::RoundtripFail
            ) {
                return Err(Error::Invalid(format!(
                    "invalid P4 parser outcome: {outcome:?}"
                )));
            }
            if expected.insert(path.clone(), outcome).is_some() {
                return Err(Error::Invalid(format!(
                    "duplicate suite input: {}",
                    path.display()
                )));
            }
        }
    }
    // The OCaml p4parse target supplies neither excludes nor patch substitution
    corpus::validate_inventory(&expected, &paths, &BTreeSet::new())?;
    eprintln!(
        "P4 parser: collected={}, excluded=0; preparing print hints",
        paths.len()
    );
    let spec_el =
        parse_files([Path::new("spec")]).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_il =
        elaborate::elaborate(spec_el).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_al = algo::convert(spec_il).map_err(|error| Error::Invalid(error.to_string()))?;
    let unparser = P4Unparser::from_al_spec(&spec_al);
    let includes = vec![PathBuf::from("p4c/p4include")];
    fs::read_dir(&includes[0])?;
    let progress = ProgressBar::new(paths.len() as u64).with_style(
        ProgressStyle::with_template("[{bar:24}] {pos}/{len} {elapsed_precise} {msg}")
            .map_err(|error| Error::Invalid(error.to_string()))?,
    );
    let mut results = Results::new(&expected);
    let mut passed = 0;
    for path in &paths {
        progress.set_message(path.display().to_string());
        let (outcome, diagnostic) = roundtrip(&unparser, &includes, path)?;
        if outcome == Outcome::Pass {
            passed += 1;
        }
        if !results.record(path, outcome)? {
            progress.suspend(|| {
                eprintln!(
                    "MISMATCH {}: expected {:?}, actual {outcome:?}",
                    path.display(),
                    expected[path]
                );
                if let Some(diagnostic) = diagnostic {
                    eprintln!("{diagnostic}");
                }
            });
        }
        progress.inc(1);
    }
    progress.finish_with_message("complete");
    eprintln!(
        "P4 parser collected={} excluded=0 executed={} pass={passed} fail={} matched={} mismatched={} elapsed={:.3}s",
        paths.len(),
        paths.len(),
        paths.len() - passed,
        results.matched,
        results.mismatched,
        start.elapsed().as_secs_f64()
    );
    results.finish()
}
