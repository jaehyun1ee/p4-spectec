//! Native AL comparisons against an uncached, independently built OCaml runner

use std::{
    collections::BTreeSet,
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    rc::Rc,
    sync::Mutex,
};

use p4spec_rust::{
    frontend::parse::parse_files,
    interface::p4::{parse::parse_file, unparse::P4Unparser},
    interp::al::{Al, Config, context::Global, error::Error},
    lang::il::ast::Typ,
    lang::{
        common::source::Span,
        data::value::{Value, make},
    },
    pass::{algo, elaborate},
    runner::{
        BuiltinInterface, Extern, ExternError, Interface, Interpreter, NullExtern, Runner,
        RunnerContext,
    },
    sim::placeholder::Placeholder,
    wire::ocaml::{
        lang::il::{ValueCodec, ValueEnvelopeCodec},
        yojson,
    },
};
use serde_json::{Value as Json, json};

static OCAML_LOCK: Mutex<()> = Mutex::new(());

fn repo() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
}

fn native<E: Extern>(
    spec: &Path,
    det: bool,
    guard: bool,
    extern_: E,
) -> Runner<Al, BuiltinInterface, E> {
    let spec_el = parse_files([spec]).expect("native specification parsing");
    let spec_il = elaborate::elaborate(spec_el).expect("native elaboration");
    let spec_al = algo::convert(spec_il).expect("native algorithmic conversion");
    let unparser = P4Unparser::from_al_spec(&spec_al);
    Runner::new(
        Global::load(spec_al).unwrap(),
        Config::new(det, guard),
        BuiltinInterface::new(unparser),
        extern_,
    )
}

struct Oracle {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
}

impl Oracle {
    fn build() {
        let output = Command::new("opam")
            .args([
                "exec",
                "--",
                "dune",
                "build",
                "p4spec/test/al-oracle/al_oracle.exe",
            ])
            .current_dir(repo())
            .output()
            .expect("build OCaml oracle");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn new(spec: &Path, det: bool, guard: bool, reentry: bool) -> Self {
        let mut child =
            Command::new(repo().join("_build/default/p4spec/test/al-oracle/al_oracle.exe"))
                .arg(spec)
                .arg(det.to_string())
                .arg(guard.to_string())
                .arg(if reentry { "reentry" } else { "p4" })
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .expect("start uncached OCaml oracle");
        let input = child.stdin.take();
        let output = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            input,
            output,
        }
    }

    fn query(&mut self, request: &Json) -> Json {
        writeln!(self.input.as_mut().unwrap(), "{request}").unwrap();
        self.input.as_mut().unwrap().flush().unwrap();
        loop {
            let mut line = String::new();
            assert_ne!(
                self.output.read_line(&mut line).unwrap(),
                0,
                "OCaml oracle terminated"
            );
            if let Some(result) = line.strip_prefix("AL-RESULT ") {
                return serde_json::from_str(result).expect("structured oracle result");
            }
            eprint!("OCaml: {line}");
        }
    }
}

impl Drop for Oracle {
    fn drop(&mut self) {
        self.input.take();
        // A failed assertion must not leave a reference interpreter running
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn success(values: Vec<Rc<Value>>) -> Json {
    json!({"status": "passed", "values": values.iter().map(|value| String::from_utf8(ValueEnvelopeCodec::encode(value).unwrap()).unwrap()).collect::<Vec<_>>()})
}

fn failure(error: Error) -> Json {
    let message = error.to_string();
    json!({"status": "runtime", "span": error.span.to_string(), "category": category(&message), "message": message})
}

fn category(message: &str) -> &'static str {
    if message.contains("non-deterministic application") {
        "nondeterminism"
    } else if message.contains("does not match the") {
        "guard"
    } else if message.contains("arity mismatch") {
        "arity"
    } else if message.contains("index") && message.contains("out of bounds") {
        "bounds"
    } else {
        "execution"
    }
}

fn describe(value: &yojson::Value) -> String {
    match value {
        yojson::Value::Assoc(fields) => format!("object with {} fields", fields.len()),
        yojson::Value::List(values) => format!("list of length {}", values.len()),
        yojson::Value::Tuple(values) => format!("tuple of length {}", values.len()),
        yojson::Value::Variant(tag, _) => format!("variant {tag}"),
        yojson::Value::String(value) if value.len() > 160 => {
            format!("{:?}…", value.chars().take(160).collect::<String>())
        }
        value => format!("{value:?}"),
    }
}

fn first_difference(
    expected: &yojson::Value,
    actual: &yojson::Value,
    path: &str,
) -> Option<String> {
    if expected == actual {
        return None;
    }
    match (expected, actual) {
        (yojson::Value::Assoc(expected), yojson::Value::Assoc(actual))
            if expected.len() == actual.len() =>
        {
            expected.iter().zip(actual).find_map(
                |((key_expected, expected), (key_actual, actual))| {
                    if key_expected != key_actual {
                        Some(format!(
                            "{path}: expected key {key_expected:?}, actual key {key_actual:?}"
                        ))
                    } else if key_expected == "at" {
                        // Source locations are excluded from semantic parity checks
                        None
                    } else {
                        first_difference(expected, actual, &format!("{path}/{key_expected}"))
                    }
                },
            )
        }
        (yojson::Value::List(expected), yojson::Value::List(actual)) if matches!(expected.first(), Some(yojson::Value::String(tag)) if tag == "ExternV") => {
            Some(format!(
                "{path}: external payload differs: {expected:?} != {actual:?}"
            ))
        }
        (yojson::Value::List(expected), yojson::Value::List(actual))
        | (yojson::Value::Tuple(expected), yojson::Value::Tuple(actual))
            if expected.len() == actual.len() =>
        {
            expected
                .iter()
                .zip(actual)
                .enumerate()
                .find_map(|(index, (expected, actual))| {
                    first_difference(expected, actual, &format!("{path}/{index}"))
                })
        }
        (
            yojson::Value::Variant(tag_expected, Some(expected)),
            yojson::Value::Variant(tag_actual, Some(actual)),
        ) if tag_expected == tag_actual => {
            first_difference(expected, actual, &format!("{path}/{tag_expected}"))
        }
        _ => Some(format!(
            "{path}: OCaml {}, Rust {}",
            describe(expected),
            describe(actual)
        )),
    }
}

fn compare(expected: Json, actual: Json, label: &str) {
    assert_eq!(
        expected["status"], actual["status"],
        "{label}\nOCaml: {}\nRust: {}",
        expected["message"], actual["message"]
    );
    if expected["status"] == "passed" {
        let expected = expected["values"].as_array().unwrap();
        let actual = actual["values"].as_array().unwrap();
        assert_eq!(
            expected.len(),
            actual.len(),
            "{label}: ordered output arity"
        );
        for (index, (expected, actual)) in expected.iter().zip(actual).enumerate() {
            // Decode the wire note, which includes OCaml-only cached hash metadata
            let expected =
                ValueEnvelopeCodec::decode(expected.as_str().unwrap().as_bytes()).unwrap();
            let actual = ValueEnvelopeCodec::decode(actual.as_str().unwrap().as_bytes()).unwrap();
            let expected =
                yojson::Value::from_slice(&ValueEnvelopeCodec::encode(&expected).unwrap()).unwrap();
            let actual =
                yojson::Value::from_slice(&ValueEnvelopeCodec::encode(&actual).unwrap()).unwrap();
            if let Some(difference) = first_difference(&expected, &actual, "") {
                panic!("{label}: output {index}, first difference {difference}");
            }
        }
    } else {
        if expected["status"] == "runtime" {
            assert_eq!(
                expected["category"], actual["category"],
                "{label}: failure category\nOCaml: {expected}\nRust: {actual}"
            );
        }
    }
}

fn fixtures(det: bool) {
    let spec = repo().join("p4spec-rust/tests/fixtures/interp/al/compare.watsup");
    for guard in [false, true] {
        let mut runner = native(&spec, det, guard, NullExtern);
        let mut oracle = Oracle::new(&spec, det, guard, false);
        let nat = make::nat(5.into(), Span::default());
        let malformed = make::bool(true, Span::default());
        for (kind, name, values) in [
            ("relation", "Ordered", vec![nat.clone()]),
            ("relation", "Choice", vec![nat]),
            ("function", "ignore", vec![malformed.clone()]),
            ("relation", "Ignore", vec![malformed]),
            ("function", "recover", vec![]),
            ("function", "bad_output", vec![]),
            ("function", "reject", vec![]),
        ] {
            let request = json!({"kind": kind, "name": name, "values": values.iter().map(|value| ValueCodec::encode(value).unwrap()).collect::<Vec<_>>()});
            let expected = oracle.query(&request);
            let result = if kind == "relation" {
                runner.eval_rel(name, &values)
            } else {
                runner
                    .eval_func(name, &[], &values)
                    .map(|value| vec![value])
            };
            let actual = result.map_or_else(failure, success);
            compare(
                expected,
                actual,
                &format!("{name}, det={det}, guard={guard}, cache=false"),
            );
        }
    }
}

struct Bridge;

impl Extern for Bridge {
    fn eval_func<S, I>(
        &self,
        context: &mut RunnerContext<'_, S, I, Self>,
        name: &str,
        targs: &[Typ],
        values: &[Rc<Value>],
    ) -> Result<(Rc<Value>, bool), S::Error>
    where
        I: Interface,
        S: Interpreter<I, Self>,
    {
        let value = match name {
            "bridge" => context.call_func("inner", targs, values)?,
            "bridge_bad" => {
                context.call_func("ignore", &[], &[make::bool(true, Span::default())])?
            }
            _ => return Err(ExternError::Failure("unknown bridge function".to_owned()).into()),
        };
        Ok((value, false))
    }

    fn eval_rel<S, I>(
        &self,
        _context: &mut RunnerContext<'_, S, I, Self>,
        _name: &str,
        _values: &[Rc<Value>],
    ) -> Result<(Vec<Rc<Value>>, bool), S::Error>
    where
        I: Interface,
        S: Interpreter<I, Self>,
    {
        Err(ExternError::Failure("unknown bridge relation".to_owned()).into())
    }

    fn clear(&mut self) {}
}

fn reentry(det: bool) {
    let spec = repo().join("p4spec-rust/tests/fixtures/interp/al/compare.watsup");
    for guard in [false, true] {
        let mut runner = native(&spec, det, guard, Bridge);
        let mut oracle = Oracle::new(&spec, det, guard, true);
        for (name, values) in [
            ("outer", vec![make::nat(5.into(), Span::default())]),
            ("outer_bad", vec![]),
            ("outer", vec![make::nat(9.into(), Span::default())]),
        ] {
            let expected = oracle.query(&json!({"kind": "function", "name": name, "values": values.iter().map(|value| ValueCodec::encode(value).unwrap()).collect::<Vec<_>>()}));
            let actual = runner
                .eval_func(name, &[], &values)
                .map(|value| vec![value])
                .map_or_else(failure, success);
            compare(
                expected,
                actual,
                &format!("reentry {name}, det={det}, guard={guard}, cache=false"),
            );
        }
    }
}

fn collect(path: &Path, extension: &str, files: &mut Vec<PathBuf>) {
    let mut entries = fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            if path.file_name().unwrap() != "include" {
                collect(&path, extension, files);
            }
        } else if path.extension().is_some_and(|value| value == extension) {
            files.push(path);
        }
    }
}

fn corpus() -> Vec<(PathBuf, &'static str)> {
    if let Some(value) = std::env::var_os("P4SPEC_AL_NARROW_FALLBACK") {
        assert_eq!(
            value, "1",
            "P4SPEC_AL_NARROW_FALLBACK accepts only explicit opt-in 1"
        );
        eprintln!(
            "Explicit narrow fallback: action-bind.p4, action-synth.p4, xor_test.p4 with Program_ok; full corpus is not being tested"
        );
        return ["action-bind.p4", "action-synth.p4", "xor_test.p4"]
            .into_iter()
            .map(|name| {
                (
                    repo().join("p4c/testdata/p4_16_samples").join(name),
                    "Program_ok",
                )
            })
            .collect();
    }
    let mut exclude_files = vec![];
    collect(
        &repo().join("excludes/static"),
        "exclude",
        &mut exclude_files,
    );
    let excludes = exclude_files
        .iter()
        .flat_map(|path| {
            fs::read_to_string(path)
                .unwrap()
                .lines()
                .filter(|line| !line.starts_with('#'))
                .map(|line| repo().join(line))
                .collect::<Vec<_>>()
        })
        .collect::<BTreeSet<_>>();
    let mut corpus = vec![];
    // The pinned run/test.ml driver does not apply patch substitution
    for (directory, relation) in [
        ("p4_16_samples", "Program_inst"),
        ("p4_16_errors", "Program_ok"),
    ] {
        let mut files = vec![];
        collect(
            &repo().join("p4c/testdata").join(directory),
            "p4",
            &mut files,
        );
        corpus.extend(
            files
                .into_iter()
                .filter(|path| !excludes.contains(path))
                .map(|path| (path, relation)),
        );
    }
    corpus
}

fn run_corpus(det: bool) {
    let _lock = OCAML_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    Oracle::build();
    fixtures(det);
    reentry(det);
    let files = corpus();
    assert!(!files.is_empty(), "supported corpus must be present");
    let spec = repo().join("spec");
    let mut runner = native(&spec, det, false, Placeholder);
    let includes = vec![repo().join("p4c/p4include")];
    for (index, (path, name)) in files.iter().enumerate() {
        eprintln!(
            "[{}/{}] {} {name}, det={det}, guard=false, cache=false",
            index + 1,
            files.len(),
            path.display()
        );
        // Isolate OCaml's process-global fresh identifiers between programs
        let mut oracle = Oracle::new(&spec, det, false, false);
        let expected = oracle
            .query(&json!({"kind": "program", "name": name, "path": path, "includes": includes}));
        runner.clear();
        let actual = match parse_file(&includes, path) {
            Ok(program) => runner
                .eval_program(name, program)
                .map_or_else(failure, success),
            Err(error) => {
                json!({"status": "syntax", "span": error.span.to_string(), "message": error.to_string()})
            }
        };
        compare(expected, actual, &path.display().to_string());
    }
}

fn with_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
#[ignore = "requires pinned OCaml toolchain; full uncached P4 corpus is slow"]
fn test_run_al_corpus_matches_ocaml() {
    with_stack(|| run_corpus(false));
}

#[test]
#[ignore = "requires pinned OCaml toolchain; full deterministic uncached P4 corpus is slow"]
fn test_run_al_det_corpus_matches_ocaml() {
    with_stack(|| run_corpus(true));
}

#[test]
#[ignore = "requires pinned OCaml toolchain"]
fn test_choice_and_guards_match_ocaml() {
    with_stack(|| {
        let _lock = OCAML_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Oracle::build();
        fixtures(false);
        fixtures(true);
        reentry(false);
        reentry(true);
    });
}

#[test]
fn test_semantic_comparison_ignores_spans_but_preserves_values_and_order() {
    let expected = yojson::Value::from_slice(br#"{"at":{"column":1},"it":[1,2]}"#).unwrap();
    let relocated = yojson::Value::from_slice(br#"{"at":{"column":9},"it":[1,2]}"#).unwrap();
    let reordered = yojson::Value::from_slice(br#"{"at":{"column":9},"it":[2,1]}"#).unwrap();
    assert!(first_difference(&expected, &relocated, "").is_none());
    assert!(
        first_difference(&expected, &reordered, "")
            .unwrap()
            .contains("/it/0")
    );
}

#[test]
fn test_semantic_comparison_preserves_external_at_fields() {
    let expected = yojson::Value::from_slice(br#"["ExternV",{"it":0,"at":1}]"#).unwrap();
    let actual = yojson::Value::from_slice(br#"["ExternV",{"it":0,"at":2}]"#).unwrap();
    assert!(first_difference(&expected, &actual, "").is_some());
}
