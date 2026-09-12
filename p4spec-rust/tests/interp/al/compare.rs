//! Native AL comparisons against an independently built OCaml runner

use p4spec_rust::lang::data::value::{ValueArena, ValueCase};
use std::{
    collections::BTreeSet,
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::Mutex,
};

use p4spec_rust::{
    frontend::parse::parse_files,
    interface::p4::{parse::parse_file, unparse::P4Unparser},
    interp::al::{Al, Config, context::Global, error::Error},
    lang::il::ast::Typ,
    lang::{
        common::{
            Iter,
            notation::{atom::Atom, mixfix::Mixfix},
            source::Span,
        },
        data::{
            typ::TypKind,
            value::{Value, ValueKind, make},
        },
        xl::num::{Number, Typ as NumTyp},
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

    fn compare_program(
        &mut self,
        arena: &ValueArena,
        request: &Json,
        actual: Result<Vec<Value>, Json>,
        label: &str,
    ) {
        self.request(request);
        let line = self.read_protocol_line();
        if let Some(result) = line.strip_prefix("AL-RESULT ") {
            let expected: Json =
                serde_json::from_str(result).expect("structured oracle failure result");
            let actual = match actual {
                Err(actual) => actual,
                Ok(_) => panic!("{label}: OCaml failed but Rust passed\nOCaml: {expected}"),
            };
            assert_ne!(
                expected["status"], "passed",
                "{label}: successful program result must use AL-STREAM"
            );
            compare(expected, actual, label);
            return;
        }

        let expected_arity = line
            .strip_prefix("AL-STREAM ")
            .expect("program oracle request must return AL-RESULT or AL-STREAM")
            .parse::<usize>()
            .expect("numeric AL-STREAM arity");
        let values = actual.unwrap_or_else(|actual| {
            panic!("{label}: OCaml passed but Rust failed\nRust: {actual}")
        });
        assert_eq!(
            expected_arity,
            values.len(),
            "{label}: ordered output arity"
        );
        for (output_index, value) in values.iter().enumerate() {
            let mut frame_index = 0;
            semantic_frames(arena, value, &mut |actual_frame| {
                let line = self.read_protocol_line();
                let expected_frame = line
                    .strip_prefix("AL-FRAME ")
                    .unwrap_or_else(|| {
                        panic!(
                            "{label}: output {output_index}, frame {frame_index}: expected OCaml frame, got {line:?}"
                        )
                    });
                let expected_frame: Json =
                    serde_json::from_str(expected_frame).expect("structured semantic oracle frame");
                assert_eq!(
                    expected_frame, actual_frame,
                    "{label}: output {output_index}, frame {frame_index}"
                );
                frame_index += 1;
            });
        }
        assert_eq!(
            self.read_protocol_line(),
            "AL-END",
            "{label}: OCaml emitted additional semantic frames"
        );
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
            let expected =
                yojson::Value::from_slice(&ValueEnvelopeCodec::encode(&arena, &expected).unwrap())
                    .unwrap();
            let actual =
                yojson::Value::from_slice(&ValueEnvelopeCodec::encode(&arena, &actual).unwrap())
                    .unwrap();
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

fn semantic_frames(arena: &ValueArena, value: &Value, emit: &mut impl FnMut(Json)) {
    let typ = semantic_type(arena.typ(value));
    match arena.kind(value) {
        ValueKind::Bool(value) => emit(json!(["Value", "Bool", typ, value])),
        ValueKind::Num(Number::Nat(value)) => emit(json!(["Value", "Nat", typ, value.to_string()])),
        ValueKind::Num(Number::Int(value)) => emit(json!(["Value", "Int", typ, value.to_string()])),
        ValueKind::Text(value) => emit(json!(["Value", "Text", typ, value])),
        ValueKind::Struct(fields) => {
            emit(json!(["Value", "Struct", typ, fields.len()]));
            for (atom, value) in fields {
                emit(json!(["Field", semantic_atom(&atom.node)]));
                semantic_frames(arena, value, emit);
            }
        }
        ValueKind::Case(value_case) => {
            emit(json!(["Value", "Case", typ]));
            semantic_mixfix_frames(arena, value_case, emit);
        }
        ValueKind::Tuple(values) => {
            emit(json!(["Value", "Tuple", typ, values.len()]));
            for value in values {
                semantic_frames(arena, value, emit);
            }
        }
        ValueKind::Opt(value) => {
            emit(json!(["Value", "Opt", typ, value.is_some()]));
            if let Some(value) = value {
                semantic_frames(arena, value, emit);
            }
        }
        ValueKind::List(values) => {
            emit(json!(["Value", "List", typ, values.len()]));
            for value in values {
                semantic_frames(arena, value, emit);
            }
        }
        ValueKind::Func(id) => emit(json!(["Value", "Func", typ, id.node])),
        ValueKind::Extern(value) => {
            emit(json!(["Value", "Extern", typ]));
            semantic_external_frames(value, emit);
        }
    }
}

fn semantic_type(typ: &TypKind) -> Json {
    match typ {
        TypKind::Bool => json!(["BoolT"]),
        TypKind::Num(NumTyp::Nat) => json!(["NumT", "NatT"]),
        TypKind::Num(NumTyp::Int) => json!(["NumT", "IntT"]),
        TypKind::Text => json!(["TextT"]),
        TypKind::Var(id, targs) => json!([
            "VarT",
            id.node,
            targs
                .iter()
                .map(|typ| semantic_type(&typ.node))
                .collect::<Vec<_>>()
        ]),
        TypKind::Tuple(types) => json!([
            "TupleT",
            types
                .iter()
                .map(|typ| semantic_type(&typ.node))
                .collect::<Vec<_>>()
        ]),
        TypKind::Iter(typ, Iter::Opt) => json!(["IterT", semantic_type(&typ.node), "Opt"]),
        TypKind::Iter(typ, Iter::List) => {
            json!(["IterT", semantic_type(&typ.node), "List"])
        }
        TypKind::Func(typ) => json!([
            "FuncT",
            typ.tparams
                .iter()
                .map(|id| id.node.as_str())
                .collect::<Vec<_>>(),
            typ.typs_params
                .iter()
                .map(|typ| semantic_type(&typ.node))
                .collect::<Vec<_>>(),
            semantic_type(&typ.typ_ret.node)
        ]),
    }
}

fn semantic_atom(atom: &Atom) -> Json {
    match atom {
        Atom::Keyword(value) => json!(["Keyword", value]),
        Atom::Tag(value) => json!(["Tag", value]),
        Atom::Operator(value) => json!(["Operator", value]),
        Atom::Sub => json!(["Sub"]),
        Atom::Sup => json!(["Sup"]),
        Atom::Turnstile => json!(["Turnstile"]),
        Atom::Tilesturn => json!(["Tilesturn"]),
        Atom::Arrow => json!(["Arrow"]),
        Atom::ArrowSub => json!(["ArrowSub"]),
        Atom::DoubleArrowSub => json!(["DoubleArrowSub"]),
        Atom::DoubleArrowLong => json!(["DoubleArrowLong"]),
        Atom::SqArrow => json!(["SqArrow"]),
        Atom::SqArrowStar => json!(["SqArrowStar"]),
        Atom::Dot => json!(["Dot"]),
        Atom::Dot2 => json!(["Dot2"]),
        Atom::Dot3 => json!(["Dot3"]),
        Atom::Semicolon => json!(["Semicolon"]),
        Atom::Colon => json!(["Colon"]),
        Atom::ColonEq => json!(["ColonEq"]),
        Atom::Tilde2 => json!(["Tilde2"]),
        Atom::Backslash => json!(["Backslash"]),
        Atom::LAngle => json!(["LAngle"]),
        Atom::RAngle => json!(["RAngle"]),
        Atom::LParen => json!(["LParen"]),
        Atom::RParen => json!(["RParen"]),
        Atom::LBrack => json!(["LBrack"]),
        Atom::RBrack => json!(["RBrack"]),
        Atom::LBrace => json!(["LBrace"]),
        Atom::RBrace => json!(["RBrace"]),
    }
}

fn semantic_mixfix_frames(arena: &ValueArena, value: &ValueCase, emit: &mut impl FnMut(Json)) {
    match value {
        Mixfix::Arg(value) => {
            emit(json!(["Mixfix", "Arg"]));
            semantic_frames(arena, value, emit);
        }
        Mixfix::Atom(atom) => {
            emit(json!(["Mixfix", "Atom", semantic_atom(&atom.node)]));
        }
        Mixfix::Brack(atom_l, mixfix_inner, atom_r) => {
            emit(json!([
                "Mixfix",
                "Brack",
                semantic_atom(&atom_l.node),
                semantic_atom(&atom_r.node)
            ]));
            semantic_mixfix_frames(arena, mixfix_inner, emit);
        }
        Mixfix::Infix(mixfix_l, atom, mixfix_r) => {
            emit(json!(["Mixfix", "Infix", semantic_atom(&atom.node)]));
            semantic_mixfix_frames(arena, mixfix_l, emit);
            semantic_mixfix_frames(arena, mixfix_r, emit);
        }
        Mixfix::Seq(values) => {
            emit(json!(["Mixfix", "Seq", values.len()]));
            for value in values {
                semantic_mixfix_frames(arena, value, emit);
            }
        }
    }
}

fn semantic_external_frames(
    value: &p4spec_rust::yojson::ExternalData,
    emit: &mut impl FnMut(Json),
) {
    use p4spec_rust::yojson::ExternalData;

    match value {
        ExternalData::Null => emit(json!(["External", "Null"])),
        ExternalData::Bool(value) => emit(json!(["External", "Bool", value])),
        ExternalData::Int(value) => emit(json!(["External", "Int", value])),
        ExternalData::Intlit(value) => emit(json!(["External", "Intlit", value])),
        ExternalData::Float(value) => emit(json!([
            "External",
            "Float",
            i64::from_ne_bytes(value.to_bits().to_ne_bytes()).to_string()
        ])),
        ExternalData::String(value) => emit(json!(["External", "String", value])),
        ExternalData::Assoc(fields) => {
            emit(json!(["External", "Assoc", fields.len()]));
            for (name, value) in fields {
                emit(json!(["ExternalField", name]));
                semantic_external_frames(value, emit);
            }
        }
        ExternalData::List(values) => {
            emit(json!(["External", "List", values.len()]));
            for value in values {
                semantic_external_frames(value, emit);
            }
        }
        ExternalData::Tuple(values) => {
            emit(json!(["External", "Tuple", values.len()]));
            for value in values {
                semantic_external_frames(value, emit);
            }
        }
        ExternalData::Variant(name, value) => {
            emit(json!(["External", "Variant", name, value.is_some()]));
            if let Some(value) = value {
                semantic_external_frames(value, emit);
            }
        }
    }
}

fn collect_semantic_frames(arena: &ValueArena, value: &Value) -> Vec<Json> {
    let mut frames = Vec::new();
    semantic_frames(arena, value, &mut |frame| frames.push(frame));
    frames
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
            "Explicit narrow fallback: xor_test.p4 with Program_inst and DASH v1model with Program_ok; full corpus is not being tested"
        );
        return [
            ("xor_test.p4", "Program_inst"),
            ("dash/dash-pipeline-v1model-bmv2.p4", "Program_ok"),
        ]
        .into_iter()
        .map(|(name, relation)| {
            (
                repo().join("p4c/testdata/p4_16_samples").join(name),
                relation,
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

fn run_corpus(cache: bool, det: bool) {
    let _lock = OCAML_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    Oracle::build();
    fixtures(cache, det);
    reentry(cache, det);
    let files = corpus();
    assert!(!files.is_empty(), "supported corpus must be present");
    let spec = repo().join("spec");
    let includes = vec![repo().join("p4c/p4include")];
    for (index, (path, name)) in files.iter().enumerate() {
        eprintln!(
            "[{}/{}] {} {name}, det={det}, guard=false, cache={cache}",
            index + 1,
            files.len(),
            path.display()
        );
        // Each program owns its runner and arena, matching the OCaml process boundary
        let mut oracle = Oracle::new(&spec, cache, det, false, false);
        let mut runner = native(&spec, cache, det, false, Placeholder);
        runner.clear();
        let actual = match parse_file(runner.arena_mut(), &includes, path) {
            Ok(program) => runner.eval_program(name, program).map_err(failure),
            Err(error) => Err(
                json!({"status": "syntax", "span": error.span.to_string(), "message": error.to_string()}),
            ),
        };
        oracle.compare_program(
            runner.arena(),
            &json!({"kind": "program", "name": name, "path": path, "includes": includes}),
            actual,
            &path.display().to_string(),
        );
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
    with_stack(|| run_corpus(false, false));
}

#[test]
#[ignore = "requires pinned OCaml toolchain; full deterministic uncached P4 corpus is slow"]
fn test_run_al_det_corpus_matches_ocaml() {
    with_stack(|| run_corpus(false, true));
}

#[test]
#[ignore = "requires pinned OCaml toolchain; full cached P4 corpus is slow"]
fn test_run_al_cached_corpus_matches_ocaml() {
    with_stack(|| run_corpus(true, false));
}

#[test]
#[ignore = "requires pinned OCaml toolchain; full deterministic cached P4 corpus is slow"]
fn test_run_al_cached_det_corpus_matches_ocaml() {
    with_stack(|| run_corpus(true, true));
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
fn test_semantic_frames_detect_nested_value_changes() {
    let mut arena = ValueArena::new();
    use p4spec_rust::lang::{data::typ, xl::num::Natural};

    let typ_list = typ::make::list(typ::make::nat());
    let original = {
        let values = vec![make::nat(&mut arena, Natural::from(1_u64), Span::default()).unwrap()];
        make::list(
            &mut arena,
            typ_list.node.clone().into(),
            values,
            Span::default(),
        )
    }
    .unwrap();
    let mutated = {
        let values = vec![make::nat(&mut arena, Natural::from(2_u64), Span::default()).unwrap()];
        make::list(
            &mut arena,
            typ_list.node.clone().into(),
            values,
            Span::default(),
        )
    }
    .unwrap();
    assert_ne!(
        collect_semantic_frames(&arena, &original),
        collect_semantic_frames(&arena, &mutated)
    );
}

#[test]
fn test_semantic_frames_detect_list_order_changes() {
    let mut arena = ValueArena::new();
    use p4spec_rust::lang::{data::typ, xl::num::Natural};

    let typ_list = typ::make::list(typ::make::nat());
    let first = make::nat(&mut arena, Natural::from(1_u64), Span::default()).unwrap();
    let second = make::nat(&mut arena, Natural::from(2_u64), Span::default()).unwrap();
    let original = make::list(
        &mut arena,
        typ_list.node.clone().into(),
        vec![first, second],
        Span::default(),
    )
    .unwrap();
    let reordered = make::list(
        &mut arena,
        typ_list.node.clone().into(),
        vec![second, first],
        Span::default(),
    )
    .unwrap();
    assert_ne!(
        collect_semantic_frames(&arena, &original),
        collect_semantic_frames(&arena, &reordered)
    );
}

#[test]
fn test_semantic_frames_detect_case_atom_changes() {
    let mut arena = ValueArena::new();
    use p4spec_rust::{
        lang::{
            common::notation::{atom::Atom, mixfix::Mixfix},
            data::typ,
        },
        phrase,
    };

    let case = |arena: &mut ValueArena, name: &str| {
        make::case(
            arena,
            (typ::make::bool()).node.clone().into(),
            Mixfix::Atom(phrase! {
                node: Atom::Keyword(name.to_owned()),
                span: Span::default(),
            }),
            Span::default(),
        )
        .unwrap()
    };
    assert_ne!(
        {
            let value = &case(&mut arena, "LEFT");
            collect_semantic_frames(&arena, value)
        },
        {
            let value = &case(&mut arena, "RIGHT");
            collect_semantic_frames(&arena, value)
        }
    );
}

#[test]
fn test_semantic_frames_distinguish_mixfix_atom_constructors() {
    let mut arena = ValueArena::new();
    use p4spec_rust::{
        lang::{
            common::notation::{atom::Atom, mixfix::Mixfix},
            data::typ,
        },
        phrase,
    };

    let value = |arena: &mut ValueArena, atom| {
        make::case(
            arena,
            (typ::make::bool()).node.clone().into(),
            Mixfix::Atom(phrase! {
                node: atom,
                span: Span::default(),
            }),
            Span::default(),
        )
        .unwrap()
    };
    assert_ne!(
        {
            let value = &value(&mut arena, Atom::Keyword("_X".to_owned()));
            collect_semantic_frames(&arena, value)
        },
        {
            let value = &value(&mut arena, Atom::Tag("X".to_owned()));
            collect_semantic_frames(&arena, value)
        }
    );
}

#[test]
fn test_semantic_frames_distinguish_struct_field_atom_constructors() {
    let mut arena = ValueArena::new();
    use p4spec_rust::lang::{common::notation::atom::Atom, data::typ};

    let value = |arena: &mut ValueArena, atom| {
        let value_bool = make::bool(arena, true, Span::default()).unwrap();
        make::structure(
            arena,
            (typ::make::bool()).node.clone().into(),
            vec![(
                p4spec_rust::phrase!(node: atom, span: Span::default()),
                value_bool,
            )],
            Span::default(),
        )
        .unwrap()
    };
    assert_ne!(
        {
            let value = &value(&mut arena, Atom::Keyword("->".to_owned()));
            collect_semantic_frames(&arena, value)
        },
        {
            let value = &value(&mut arena, Atom::Arrow);
            collect_semantic_frames(&arena, value)
        }
    );
}

#[test]
fn test_semantic_frames_detect_external_payload_and_order_changes() {
    let mut arena = ValueArena::new();
    use p4spec_rust::{lang::data::typ, yojson::ExternalData};

    let external = |arena: &mut ValueArena, fields| {
        make::external(
            arena,
            (typ::make::bool()).node.clone().into(),
            ExternalData::Assoc(fields),
            Span::default(),
        )
        .unwrap()
    };
    let original = external(
        &mut arena,
        vec![
            ("a".to_owned(), ExternalData::Int(1)),
            ("b".to_owned(), ExternalData::Int(2)),
        ],
    );
    let reordered = external(
        &mut arena,
        vec![
            ("b".to_owned(), ExternalData::Int(2)),
            ("a".to_owned(), ExternalData::Int(1)),
        ],
    );
    let mutated = external(
        &mut arena,
        vec![
            ("a".to_owned(), ExternalData::Int(1)),
            ("b".to_owned(), ExternalData::Int(3)),
        ],
    );
    let frames = collect_semantic_frames(&arena, &original);
    assert_ne!(frames, collect_semantic_frames(&arena, &reordered));
    assert_ne!(frames, collect_semantic_frames(&arena, &mutated));
}

#[test]
fn test_semantic_frames_detect_type_changes() {
    let mut arena = ValueArena::new();
    use p4spec_rust::lang::data::{typ::TypKind, value::ValueKind};

    let bool_value = make::new(
        &mut arena,
        ValueKind::Bool(true),
        TypKind::Bool.into(),
        Span::default(),
    )
    .unwrap();
    let text_typed = make::new(
        &mut arena,
        ValueKind::Bool(true),
        TypKind::Text.into(),
        Span::default(),
    )
    .unwrap();
    assert_ne!(
        collect_semantic_frames(&arena, &bool_value),
        collect_semantic_frames(&arena, &text_typed)
    );
}

#[test]
fn test_semantic_frames_ignore_source_spans() {
    let mut arena = ValueArena::new();
    use p4spec_rust::lang::common::source::Position;

    let original = make::bool(&mut arena, true, Span::default()).unwrap();
    let relocated = make::bool(
        &mut arena,
        true,
        Span::new(
            Position::new("other.p4", 10, 20),
            Position::new("other.p4", 10, 24),
        ),
    )
    .unwrap();
    assert_eq!(
        collect_semantic_frames(&arena, &original),
        collect_semantic_frames(&arena, &relocated)
    );
}

#[test]
fn test_semantic_comparison_preserves_external_at_fields() {
    let expected = yojson::Value::from_slice(br#"["ExternV",{"it":0,"at":1}]"#).unwrap();
    let actual = yojson::Value::from_slice(br#"["ExternV",{"it":0,"at":2}]"#).unwrap();
    assert!(first_difference(&expected, &actual, "").is_some());
}
