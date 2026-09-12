use crate::{
    Error, Result,
    corpus::{self, Outcome, Results},
    progress::Progress,
};
use p4spec_rust::{
    frontend::parse::parse_files,
    interface::p4::{error::P4ErrorKind, parse::parse_file, unparse::P4Unparser},
    interp::al::{Al, Config, context::Global},
    pass::{algo, elaborate},
    runner::{BuiltinInterface, Runner},
    sim::placeholder::Placeholder,
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

pub fn run() -> Result<()> {
    let start = Instant::now();
    let excludes = corpus::collect_excludes(Path::new("excludes/static"))?;
    let mut expected = BTreeMap::new();
    let mut cases = Vec::new();
    for (directory, relation, name) in [
        ("p4_16_samples", "Program_inst", "run-pos-al.expected"),
        ("p4_16_errors", "Program_ok", "run-neg-al.expected"),
    ] {
        let text = fs::read_to_string(Path::new("p4spec-rust/test-driver/expected").join(name))?;
        let records = corpus::parse_expected(&text)?;
        let paths = corpus::collect(&Path::new("p4c/testdata").join(directory), ".p4")?;
        corpus::validate_inventory(&records, &paths, &excludes)?;
        for (path, outcome) in records {
            if expected.insert(path.clone(), outcome).is_some() {
                return Err(Error::Invalid(format!(
                    "duplicate suite input: {}",
                    path.display()
                )));
            }
        }
        cases.extend(paths.into_iter().map(|path| (path, relation)));
    }
    let excluded = expected
        .values()
        .filter(|outcome| **outcome == Outcome::Exclude)
        .count();
    eprintln!(
        "AL default mode: collected={}, excluded={}, to execute={}; preparing specification",
        cases.len(),
        excluded,
        cases.len() - excluded
    );
    let spec_el =
        parse_files([Path::new("spec")]).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_il =
        elaborate::elaborate(spec_el).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_al = algo::convert(spec_il).map_err(|error| Error::Invalid(error.to_string()))?;
    let unparser = P4Unparser::from_al_spec(&spec_al);
    let global = Global::load(spec_al).map_err(|error| Error::Invalid(error.to_string()))?;
    let mut runner = Runner::<Al, _, _>::new(
        global,
        Config::new(true, false, false),
        BuiltinInterface::new(unparser),
        Placeholder,
    );
    let includes = vec![PathBuf::from("p4c/p4include")];
    fs::read_dir(&includes[0])?;
    let mut results = Results::new(&expected);
    let progress = Progress::new(cases.len());
    let mut executed = 0;
    let mut passed = 0;
    for (idx, (path, relation)) in cases.iter().enumerate() {
        progress.show(idx, &path.display().to_string());
        let outcome = if expected[path] == Outcome::Exclude {
            Outcome::Exclude
        } else {
            // Reset before parsing: no value may cross this program boundary
            runner.reset();
            fs::File::open(path)?;
            let outcome = match parse_file(runner.arena_mut(), &includes, path) {
                Ok(program) => match runner.eval_program(relation, program) {
                    Ok(_) => Outcome::Pass,
                    Err(error) => {
                        if expected[path] != Outcome::Fail {
                            eprintln!("{}: {error}", path.display());
                        }
                        Outcome::Fail
                    }
                },
                Err(error) => match error.kind {
                    P4ErrorKind::Lex(_) | P4ErrorKind::Syntax => Outcome::Fail,
                    _ => {
                        return Err(Error::Invalid(format!(
                            "{}: test execution error: {error}",
                            path.display()
                        )));
                    }
                },
            };
            executed += 1;
            if outcome == Outcome::Pass {
                passed += 1;
            }
            outcome
        };
        if !results.record(path, outcome)? {
            eprintln!(
                "MISMATCH {}: expected {:?}, actual {outcome:?}",
                path.display(),
                expected[path]
            );
        }
    }
    progress.show(cases.len(), "complete");
    eprintln!(
        "AL collected={} excluded={excluded} executed={executed} pass={passed} fail={} matched={} mismatched={} elapsed={:.3}s",
        cases.len(),
        executed - passed,
        results.matched,
        results.mismatched,
        start.elapsed().as_secs_f64()
    );
    results.finish()
}
