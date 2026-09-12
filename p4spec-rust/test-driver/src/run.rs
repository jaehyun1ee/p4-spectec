use crate::{
    Error, Result,
    corpus::{self, Outcome, Results},
};
use expect_test::expect_file;
use indicatif::{ProgressBar, ProgressStyle};
use p4spec_rust::{
    frontend::parse::parse_files,
    interface::p4::{error::P4ErrorKind, parse::parse_file, unparse::P4Unparser},
    interp::al::{Al, Config, context::Global},
    pass::{algo, elaborate},
    runner::{BuiltinInterface, Runner},
    sim::placeholder::Placeholder,
};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

pub fn run() -> Result<()> {
    let start = Instant::now();
    let excludes = corpus::collect_excludes(Path::new("excludes/static"))?;
    let mut suites = Vec::new();
    for (directory, relation, name) in [
        ("p4_16_samples", "Program_inst", "run-pos-al.expected"),
        ("p4_16_errors", "Program_ok", "run-neg-al.expected"),
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("expected")
            .join(name);
        let paths = corpus::collect(&Path::new("p4c/testdata").join(directory), ".p4")?;
        suites.push((paths, relation, Results::new(expect_file![path])));
    }
    let collected: usize = suites.iter().map(|(paths, _, _)| paths.len()).sum();
    let excluded = suites
        .iter()
        .flat_map(|(paths, _, _)| paths)
        .filter(|path| path.to_str().is_some_and(|path| excludes.contains(path)))
        .count();
    eprintln!(
        "AL default mode: collected={collected}, excluded={excluded}, to execute={}; preparing specification",
        collected - excluded
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
    let progress = ProgressBar::new(collected as u64).with_style(
        ProgressStyle::with_template("[{bar:24}] {pos}/{len} {elapsed_precise} {msg}")
            .map_err(|error| Error::Invalid(error.to_string()))?,
    );
    let mut executed = 0;
    let mut passed = 0;
    for (paths, relation, results) in &mut suites {
        for path in paths.iter() {
            progress.set_message(path.display().to_string());
            let outcome = if path.to_str().is_some_and(|path| excludes.contains(path)) {
                Outcome::Exclude
            } else {
                // Reset before parsing: no value may cross this program boundary
                runner.reset();
                fs::File::open(path)?;
                let outcome = match parse_file(runner.arena_mut(), &includes, path) {
                    Ok(program) => match runner.eval_program(relation, program) {
                        Ok(_) => Outcome::Pass,
                        Err(_) => Outcome::Fail,
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
            results.record(path, outcome)?;
            progress.inc(1);
        }
    }
    progress.finish_with_message("complete");
    eprintln!(
        "AL collected={collected} excluded={excluded} executed={executed} pass={passed} fail={} elapsed={:.3}s",
        executed - passed,
        start.elapsed().as_secs_f64()
    );
    for (_, _, results) in suites {
        results.check();
    }
    eprintln!("AL: all {collected} file results matched expected");
    Ok(())
}
