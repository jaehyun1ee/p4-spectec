use crate::{
    Error, Result,
    corpus::{self, Outcome, Results},
};
use expect_test::expect_file;
use indicatif::{ProgressBar, ProgressStyle};
use p4spec_rust::{
    frontend::parse::parse_files,
    interface::{
        self,
        p4::{error::P4ErrorKind, parse::parse_file},
    },
    interp::{
        al::{AlInterp, Config, context::Global},
        sl::{Config as SlConfig, SlInterp, context::Global as SlGlobal},
    },
    lang::al,
    pass::{algo, elaborate, structure},
    runner::{BuiltinInterface, Interpreter, Runner},
    sim_plugin::dummy::Dummy,
};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

struct RunSuite {
    paths: Vec<PathBuf>,
    id_relation: &'static str,
    use_excludes: bool,
    results: Results,
}

fn collect_suite(
    path_dir: &str,
    id_relation: &'static str,
    name_expected: &str,
    use_excludes: bool,
) -> Result<RunSuite> {
    let path_expected = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("expected")
        .join(name_expected);
    Ok(RunSuite {
        paths: corpus::collect(Path::new(path_dir), ".p4")?,
        id_relation,
        use_excludes,
        results: Results::new(expect_file![path_expected]),
    })
}

pub fn run() -> Result<()> {
    let suites = [
        collect_suite(
            "p4c/testdata/p4_16_samples",
            "Program_inst",
            "run-pos-al.expected",
            true,
        ),
        collect_suite(
            "p4c/testdata/p4_16_errors",
            "Program_ok",
            "run-neg-al.expected",
            true,
        ),
    ]
    .into_iter()
    .collect::<Result<Vec<_>>>()?;
    run_with("AL cache=on det=false", suites, |spec_al| {
        let interface = interface::p4(&spec_al);
        let global = Global::load(spec_al).map_err(|error| Error::Invalid(error.to_string()))?;
        Ok(Runner::new(
            global,
            AlInterp::new(Config::new(true, false, false)),
            interface,
            Dummy,
        ))
    })
}

/// Runs the SL execution suites against source-derived expected results
pub fn run_sl(det: bool) -> Result<()> {
    // The OCaml SL outcomes are byte-identical to these AL expectation files
    let suites = [
        collect_suite(
            "p4c/testdata/p4_16_samples",
            "Program_inst",
            "run-pos-al.expected",
            true,
        ),
        collect_suite(
            "p4c/testdata/p4_16_errors",
            "Program_ok",
            "run-neg-al.expected",
            true,
        ),
        collect_suite(
            "testdata/regression/neg",
            "Program_ok",
            "run-regression-neg-sl.expected",
            false,
        ),
        collect_suite(
            "testdata/regression/pos",
            "Program_inst",
            "run-regression-pos-sl.expected",
            false,
        ),
    ]
    .into_iter()
    .collect::<Result<Vec<_>>>()?;
    run_with(&format!("SL cache=on det={det}"), suites, |spec_al| {
        let interface = interface::p4(&spec_al);
        let without_rule_groups = true;
        let spec_sl = structure::convert(spec_al, without_rule_groups)
            .map_err(|error| Error::Invalid(error.to_string()))?;
        let global = SlGlobal::load(spec_sl).map_err(|error| Error::Invalid(error.to_string()))?;
        Ok(Runner::new(
            global,
            SlInterp::new(SlConfig::new(true, det, false)),
            interface,
            Dummy,
        ))
    })
}

fn run_with<Interp, Build>(
    text_mode: &str,
    mut suites: Vec<RunSuite>,
    build_runner: Build,
) -> Result<()>
where
    Interp: Interpreter<BuiltinInterface, Dummy>,
    Build: FnOnce(al::ast::Spec) -> Result<Runner<Interp, BuiltinInterface, Dummy>>,
{
    let start = Instant::now();
    let excludes = corpus::collect_excludes(Path::new("excludes/static"))?;
    let collected: usize = suites.iter().map(|suite| suite.paths.len()).sum();
    let excluded = suites
        .iter()
        .filter(|suite| suite.use_excludes)
        .flat_map(|suite| &suite.paths)
        .filter(|path| path.to_str().is_some_and(|path| excludes.contains(path)))
        .count();
    eprintln!(
        "{text_mode}: collected={collected} excluded={excluded} to execute={}; preparing specification",
        collected - excluded
    );
    let spec_el =
        parse_files([Path::new("spec")]).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_il = elaborate::convert(spec_el).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_al = algo::convert(spec_il).map_err(|error| Error::Invalid(error.to_string()))?;
    let mut runner = build_runner(spec_al)?;
    let includes = vec![PathBuf::from("p4c/p4include")];
    fs::read_dir(&includes[0])?;
    let progress = ProgressBar::new(collected as u64).with_style(
        ProgressStyle::with_template("[{bar:24}] {pos}/{len} {elapsed_precise} {msg}")
            .map_err(|error| Error::Invalid(error.to_string()))?,
    );
    let mut executed = 0;
    let mut passed = 0;
    for suite in &mut suites {
        for path in &suite.paths {
            progress.set_message(path.display().to_string());
            let excluded =
                suite.use_excludes && path.to_str().is_some_and(|path| excludes.contains(path));
            let outcome = if excluded {
                Outcome::Exclude
            } else {
                // Reset before parsing: no value may cross this program boundary
                runner.reset();
                fs::File::open(path)?;
                let outcome = match parse_file(runner.arena_mut(), &includes, path) {
                    Ok(program) => match runner.eval_program(suite.id_relation, program) {
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
            suite.results.record(path, outcome)?;
            progress.inc(1);
        }
    }
    progress.finish_with_message("complete");
    eprintln!(
        "{text_mode}: collected={collected} excluded={excluded} executed={executed} pass={passed} fail={} elapsed={:.3}s",
        executed - passed,
        start.elapsed().as_secs_f64()
    );
    for suite in suites {
        suite.results.check();
    }
    eprintln!("{text_mode}: all {collected} file results matched expected");
    Ok(())
}
