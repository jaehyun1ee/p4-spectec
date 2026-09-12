//! Native AL comparisons against an independently built OCaml runner

use p4spec_rust::lang::data::value::ValueArena;
use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::Mutex,
};

use p4spec_rust::{
    frontend::parse::parse_files,
    interface::p4::unparse::P4Unparser,
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
    wire::ocaml::lang::il::{ValueCodec, ValueEnvelopeCodec},
};
use serde_json::{Value as Json, json};

static OCAML_LOCK: Mutex<()> = Mutex::new(());

fn repo() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
}

fn native<E: Extern>(
    spec: &Path,
    cache: bool,
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
        Config::new(cache, det, guard),
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
            .args(["exec", "--", "dune", "build", "--root"])
            .arg(repo())
            .arg("p4spec/test/al-oracle/al_oracle.exe")
            .current_dir(repo())
            .output()
            .expect("build OCaml oracle");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn new(spec: &Path, cache: bool, det: bool, guard: bool, reentry: bool) -> Self {
        let mut child =
            Command::new(repo().join("_build/default/p4spec/test/al-oracle/al_oracle.exe"))
                .arg(spec)
                .arg(det.to_string())
                .arg(guard.to_string())
                .arg(if reentry { "reentry" } else { "p4" })
                .arg(cache.to_string())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .expect("start OCaml oracle");
        let input = child.stdin.take();
        let output = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            input,
            output,
        }
    }

    fn request(&mut self, request: &Json) {
        writeln!(self.input.as_mut().unwrap(), "{request}").unwrap();
        self.input.as_mut().unwrap().flush().unwrap();
    }

    fn read_protocol_line(&mut self) -> String {
        loop {
            let mut line = String::new();
            assert_ne!(
                self.output.read_line(&mut line).unwrap(),
                0,
                "OCaml oracle terminated"
            );
            if line.starts_with("AL-") {
                return line.trim_end().to_owned();
            }
            eprint!("OCaml: {line}");
        }
    }

    fn query(&mut self, request: &Json) -> Json {
        self.request(request);
        let line = self.read_protocol_line();
        let result = line
            .strip_prefix("AL-RESULT ")
            .expect("ordinary oracle request must return AL-RESULT");
        serde_json::from_str(result).expect("structured oracle result")
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

fn success(arena: &ValueArena, values: Vec<Value>) -> Json {
    json!({"status": "passed", "values": values.iter().map(|value| String::from_utf8(ValueEnvelopeCodec::encode(arena, value).unwrap()).unwrap()).collect::<Vec<_>>()})
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

fn describe(value: &Json) -> String {
    match value {
        Json::Object(fields) => format!("object with {} fields", fields.len()),
        Json::Array(values) => format!("list of length {}", values.len()),
        Json::String(value) if value.len() > 160 => {
            format!("{:?}…", value.chars().take(160).collect::<String>())
        }
        value => format!("{value:?}"),
    }
}

fn first_difference(expected: &Json, actual: &Json, path: &str) -> Option<String> {
    if expected == actual {
        return None;
    }
    match (expected, actual) {
        (Json::Object(expected), Json::Object(actual)) if expected.len() == actual.len() => {
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
        (Json::Array(expected), Json::Array(actual)) if expected.len() == actual.len() => expected
            .iter()
            .zip(actual)
            .enumerate()
            .find_map(|(index, (expected, actual))| {
                first_difference(expected, actual, &format!("{path}/{index}"))
            }),
        _ => Some(format!(
            "{path}: OCaml {}, Rust {}",
            describe(expected),
            describe(actual)
        )),
    }
}

fn compare(expected: Json, actual: Json, label: &str) {
    let mut arena = ValueArena::new();
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
                ValueEnvelopeCodec::decode(&mut arena, expected.as_str().unwrap().as_bytes())
                    .unwrap();
            let actual =
                ValueEnvelopeCodec::decode(&mut arena, actual.as_str().unwrap().as_bytes())
                    .unwrap();
            let expected = ValueCodec::encode(&arena, &expected).unwrap();
            let actual = ValueCodec::encode(&arena, &actual).unwrap();
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

fn fixtures(cache: bool, det: bool) {
    let spec = repo().join("p4spec-rust/tests/fixtures/interp/al/compare.watsup");
    for guard in [false, true] {
        let mut runner = native(&spec, cache, det, guard, NullExtern);
        let mut oracle = Oracle::new(&spec, cache, det, guard, false);
        let nat = make::nat(runner.arena_mut(), 5.into(), Span::default()).unwrap();
        let malformed = make::bool(runner.arena_mut(), true, Span::default()).unwrap();
        for (kind, name, values) in [
            ("relation", "Ordered", vec![nat]),
            ("relation", "Choice", vec![nat]),
            ("function", "ignore", vec![malformed]),
            ("relation", "Ignore", vec![malformed]),
            ("function", "recover", vec![]),
            ("function", "bad_output", vec![]),
            ("function", "reject", vec![]),
        ] {
            let request = json!({"kind": kind, "name": name, "values": values.iter().map(|value| ValueCodec::encode(runner.arena(), value).unwrap()).collect::<Vec<_>>()});
            let expected = oracle.query(&request);
            let result = if kind == "relation" {
                runner.eval_rel(name, &values)
            } else {
                runner
                    .eval_func(name, &[], &values)
                    .map(|value| vec![value])
            };
            let actual = result.map_or_else(failure, |values| success(runner.arena(), values));
            compare(
                expected,
                actual,
                &format!("{name}, det={det}, guard={guard}, cache={cache}"),
            );
        }
    }
}

struct Bridge;

impl Extern for Bridge {
    fn eval_func<S, I>(
        &self,
        ctx: &mut RunnerContext<'_, S, I, Self>,
        name: &str,
        targs: &[Typ],
        values: &[Value],
    ) -> Result<(Value, bool), S::Error>
    where
        I: Interface,
        S: Interpreter<I, Self>,
    {
        let value = match name {
            "bridge" => ctx.call_func("inner", targs, values)?,
            "bridge_bad" => {
                let (name, targs, values) = (
                    "ignore",
                    &[],
                    &[make::bool(ctx.arena_mut(), true, Span::default()).unwrap()],
                );
                ctx.call_func(name, targs, values)
            }?,
            _ => return Err(ExternError::Failure("unknown bridge function".to_owned()).into()),
        };
        Ok((value, false))
    }

    fn eval_rel<S, I>(
        &self,
        _context: &mut RunnerContext<'_, S, I, Self>,
        _name: &str,
        _values: &[Value],
    ) -> Result<(Vec<Value>, bool), S::Error>
    where
        I: Interface,
        S: Interpreter<I, Self>,
    {
        Err(ExternError::Failure("unknown bridge relation".to_owned()).into())
    }

    fn clear(&mut self) {}
}

fn reentry(cache: bool, det: bool) {
    let spec = repo().join("p4spec-rust/tests/fixtures/interp/al/compare.watsup");
    for guard in [false, true] {
        let mut runner = native(&spec, cache, det, guard, Bridge);
        let mut oracle = Oracle::new(&spec, cache, det, guard, true);
        for (name, values) in [
            (
                "outer",
                vec![make::nat(runner.arena_mut(), 5.into(), Span::default()).unwrap()],
            ),
            ("outer_bad", vec![]),
            (
                "outer",
                vec![make::nat(runner.arena_mut(), 9.into(), Span::default()).unwrap()],
            ),
        ] {
            let expected = oracle.query(&json!({"kind": "function", "name": name, "values": values.iter().map(|value| ValueCodec::encode(runner.arena(), value).unwrap()).collect::<Vec<_>>()}));
            let actual = runner
                .eval_func(name, &[], &values)
                .map(|value| vec![value])
                .map_or_else(failure, |values| success(runner.arena(), values));
            compare(
                expected,
                actual,
                &format!("reentry {name}, det={det}, guard={guard}, cache={cache}"),
            );
        }
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
#[ignore = "requires pinned OCaml toolchain"]
fn test_choice_and_guards_match_ocaml() {
    with_stack(|| {
        let _lock = OCAML_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Oracle::build();
        for cache in [false, true] {
            fixtures(cache, false);
            fixtures(cache, true);
            reentry(cache, false);
            reentry(cache, true);
        }
    });
}

#[test]
#[ignore = "requires pinned OCaml toolchain"]
fn test_oracle_cache_flag_controls_public_input_guard() {
    with_stack(|| {
        let _lock = OCAML_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Oracle::build();
        let spec = repo().join("p4spec-rust/tests/fixtures/interp/al/compare.watsup");
        for cache in [false, true] {
            let mut runner = native(&spec, cache, false, true, NullExtern);
            let mut oracle = Oracle::new(&spec, cache, false, true, false);
            let value = make::bool(runner.arena_mut(), true, Span::default()).unwrap();
            let expected = oracle.query(&json!({
                "kind": "function", "name": "ignore",
                "values": [ValueCodec::encode(runner.arena(), &value).unwrap()],
            }));
            assert_eq!(expected["status"] == "passed", cache);
            let result = runner.eval_func("ignore", &[], &[value]);
            assert_eq!(result.is_ok(), cache);
            let actual = result.map_or_else(failure, |value| success(runner.arena(), vec![value]));
            compare(
                expected,
                actual,
                &format!("public input guard, cache={cache}"),
            );
        }
    });
}

#[test]
fn test_semantic_comparison_ignores_spans_but_preserves_values_and_order() {
    let expected = serde_json::from_slice::<Json>(br#"{"at":{"column":1},"it":[1,2]}"#).unwrap();
    let relocated = serde_json::from_slice::<Json>(br#"{"at":{"column":9},"it":[1,2]}"#).unwrap();
    let reordered = serde_json::from_slice::<Json>(br#"{"at":{"column":9},"it":[2,1]}"#).unwrap();
    assert!(first_difference(&expected, &relocated, "").is_none());
    assert!(
        first_difference(&expected, &reordered, "")
            .unwrap()
            .contains("/it/0")
    );
}
