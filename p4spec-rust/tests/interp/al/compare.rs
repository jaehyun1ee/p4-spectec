//! Native AL comparisons against an independently built OCaml runner

use p4spec_rust::lang::data::value::ValueArena;
use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::Mutex,
};

use p4spec_rust::util::json::json;
use p4spec_rust::{
    frontend::parse::parse_files,
    interface::p4::unparse::P4Unparser,
    interp::al::{AlInterp, Config, context::Global, error::Error},
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
use serde_json::json;

static OCAML_LOCK: Mutex<()> = Mutex::new(());

fn repo() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
}

fn native<Exn: Extern>(
    spec: &Path,
    cache: bool,
    det: bool,
    guard: bool,
    external: Exn,
) -> Runner<AlInterp, BuiltinInterface, Exn> {
    let spec_el = parse_files([spec]).expect("native specification parsing");
    let spec_il = elaborate::elaborate(spec_el).expect("native elaboration");
    let spec_al = algo::convert(spec_il).expect("native algorithmic conversion");
    let unparser = P4Unparser::from_al_spec(&spec_al);
    Runner::new(
        Global::load(spec_al).unwrap(),
        AlInterp::new(Config::new(cache, det, guard)),
        BuiltinInterface::new(unparser),
        external,
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

    fn request(&mut self, json_request: &json) {
        writeln!(self.input.as_mut().unwrap(), "{json_request}").unwrap();
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

    fn query(&mut self, json_request: &json) -> json {
        self.request(json_request);
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

fn success(arena: &ValueArena, values: Vec<Value>) -> json {
    json!({"status": "passed", "values": values.iter().map(|value| String::from_utf8(ValueEnvelopeCodec::encode(arena, value).unwrap()).unwrap()).collect::<Vec<_>>()})
}

fn failure(error: Error) -> json {
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

fn describe(json: &json) -> String {
    match json {
        json::Object(fields) => format!("object with {} fields", fields.len()),
        json::Array(jsons) => format!("list of length {}", jsons.len()),
        json::String(text) if text.len() > 160 => {
            format!("{:?}…", text.chars().take(160).collect::<String>())
        }
        json => format!("{json:?}"),
    }
}

fn first_difference(json_expected: &json, json_actual: &json, path: &str) -> Option<String> {
    if json_expected == json_actual {
        return None;
    }
    match (json_expected, json_actual) {
        (json::Object(fields_expected), json::Object(fields_actual))
            if fields_expected.len() == fields_actual.len() =>
        {
            fields_expected.iter().zip(fields_actual).find_map(
                |((key_expected, json_expected), (key_actual, json_actual))| {
                    if key_expected != key_actual {
                        Some(format!(
                            "{path}: expected key {key_expected:?}, actual key {key_actual:?}"
                        ))
                    } else if key_expected == "at" {
                        // Source locations are excluded from semantic parity checks
                        None
                    } else {
                        first_difference(
                            json_expected,
                            json_actual,
                            &format!("{path}/{key_expected}"),
                        )
                    }
                },
            )
        }
        (json::Array(jsons_expected), json::Array(jsons_actual))
            if jsons_expected.len() == jsons_actual.len() =>
        {
            jsons_expected
                .iter()
                .zip(jsons_actual)
                .enumerate()
                .find_map(|(index, (json_expected, json_actual))| {
                    first_difference(json_expected, json_actual, &format!("{path}/{index}"))
                })
        }
        _ => Some(format!(
            "{path}: OCaml {}, Rust {}",
            describe(json_expected),
            describe(json_actual)
        )),
    }
}

fn compare(json_expected: json, json_actual: json, label: &str) {
    let mut arena = ValueArena::new();
    assert_eq!(
        json_expected["status"], json_actual["status"],
        "{label}\nOCaml: {}\nRust: {}",
        json_expected["message"], json_actual["message"]
    );
    if json_expected["status"] == "passed" {
        let jsons_expected = json_expected["values"].as_array().unwrap();
        let jsons_actual = json_actual["values"].as_array().unwrap();
        assert_eq!(
            jsons_expected.len(),
            jsons_actual.len(),
            "{label}: ordered output arity"
        );
        for (index, (json_expected, json_actual)) in
            jsons_expected.iter().zip(jsons_actual).enumerate()
        {
            // Decode the wire note, which includes OCaml-only cached hash metadata
            let value_expected =
                ValueEnvelopeCodec::decode(&mut arena, json_expected.as_str().unwrap().as_bytes())
                    .unwrap();
            let value_actual =
                ValueEnvelopeCodec::decode(&mut arena, json_actual.as_str().unwrap().as_bytes())
                    .unwrap();
            let json_expected = ValueCodec::encode(&arena, &value_expected).unwrap();
            let json_actual = ValueCodec::encode(&arena, &value_actual).unwrap();
            if let Some(difference) = first_difference(&json_expected, &json_actual, "") {
                panic!("{label}: output {index}, first difference {difference}");
            }
        }
    } else {
        if json_expected["status"] == "runtime" {
            assert_eq!(
                json_expected["category"], json_actual["category"],
                "{label}: failure category\nOCaml: {json_expected}\nRust: {json_actual}"
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
    fn eval_func<Interp, Iface>(
        &self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        name: &str,
        targs: &[Typ],
        values: &[Value],
    ) -> Result<(Value, bool), Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
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

    fn eval_rel<Interp, Iface>(
        &self,
        _ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        _name: &str,
        _values: &[Value],
    ) -> Result<(Vec<Value>, bool), Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
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
    let expected = serde_json::from_slice::<json>(br#"{"at":{"column":1},"it":[1,2]}"#).unwrap();
    let relocated = serde_json::from_slice::<json>(br#"{"at":{"column":9},"it":[1,2]}"#).unwrap();
    let reordered = serde_json::from_slice::<json>(br#"{"at":{"column":9},"it":[2,1]}"#).unwrap();
    assert!(first_difference(&expected, &relocated, "").is_none());
    assert!(
        first_difference(&expected, &reordered, "")
            .unwrap()
            .contains("/it/0")
    );
}
