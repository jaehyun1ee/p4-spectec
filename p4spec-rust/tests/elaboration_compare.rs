use std::{
    path::Path,
    process::{Command, Output},
};

use p4spec_rust::{
    frontend::parse::parse_files,
    pass::elaborate::{self, ElabErrorKind},
    wire::{
        Envelope, IL_SCHEMA,
        ocaml::{lang::il::SpecCodec, source},
    },
};
use serde_json::Value;

fn first_difference(left: &Value, right: &Value, path: &str) -> Option<(String, String, String)> {
    if left == right {
        return None;
    }
    match (left, right) {
        (Value::Array(left), Value::Array(right)) if left.len() == right.len() => left
            .iter()
            .zip(right)
            .enumerate()
            .find_map(|(index, (left, right))| {
                first_difference(left, right, &format!("{path}[{index}]"))
            }),
        (Value::Object(left), Value::Object(right)) if left.len() == right.len() => {
            left.iter().find_map(|(key, left)| {
                right
                    .get(key)
                    .and_then(|right| first_difference(left, right, &format!("{path}.{key}")))
            })
        }
        _ => Some((path.to_owned(), left.to_string(), right.to_string())),
    }
}

fn run_ocaml_oracle(repo: &Path, spec_path: &Path) -> Output {
    Command::new("opam")
        .args([
            "exec",
            "--",
            "dune",
            "exec",
            "--root",
            repo.to_str().expect("UTF-8 repository path"),
            "./p4spec/test/g05-oracle/g05_oracle.exe",
            "--",
            spec_path.to_str().expect("UTF-8 specification path"),
        ])
        .current_dir(repo)
        .output()
        .expect("run pinned OCaml elaboration oracle")
}

#[test]
#[ignore = "requires the pinned OCaml toolchain"]
fn full_il_matches_ocaml_exactly() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Rust crate is inside the repository");
    let spec_path = std::env::var_os("P4SPEC_TEST_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| repo.join("spec"));
    let output = run_ocaml_oracle(repo, &spec_path);
    assert!(
        output.status.success(),
        "OCaml elaboration failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected = Envelope::<Value>::from_slice(&output.stdout).expect("decode IL envelope");
    assert_eq!(expected.schema(), IL_SCHEMA);
    assert_eq!(expected.kind(), "il");

    let spec = parse_files([&spec_path]).expect("parse corpus with Rust frontend");
    let spec = elaborate::elaborate(&spec).expect("elaborate corpus with Rust");
    let actual = SpecCodec::encode(&spec).expect("encode Rust IL");

    if let Some((path, expected, actual)) = first_difference(expected.payload(), &actual, "payload")
    {
        panic!("first IL mismatch at {path}:\nOCaml: {expected}\nRust:  {actual}");
    }
}

fn rust_error_category(kind: &ElabErrorKind) -> &'static str {
    match kind {
        ElabErrorKind::Undefined(_) | ElabErrorKind::Type(_) => "undefined",
        ElabErrorKind::Duplicate(_) => "duplicate",
        ElabErrorKind::ArityMismatch => "arity_mismatch",
        ElabErrorKind::CannotInfer => "cannot_infer",
        ElabErrorKind::InvalidCast => "invalid_cast",
        ElabErrorKind::OperatorNotDefined => "operator_not_defined",
        ElabErrorKind::InvalidIdentifier => "invalid_identifier",
        _ => "invalid_definition",
    }
}

#[test]
#[ignore = "requires the pinned OCaml toolchain"]
fn rejected_elaboration_matches_ocaml_category_and_span() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Rust crate is inside the repository");
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/elaboration");

    for name in ["duplicate_metavariable.watsup", "undefined_function.watsup"] {
        let fixture = fixtures.join(name);
        let output = run_ocaml_oracle(repo, &fixture);
        assert!(!output.status.success(), "OCaml accepted {name}");
        let diagnostic: Value =
            serde_json::from_slice(&output.stdout).expect("decode OCaml elaboration diagnostic");
        let category = diagnostic["category"]
            .as_str()
            .expect("diagnostic category");
        let span = source::decode_region(&diagnostic["span"]).expect("diagnostic span");

        let spec = parse_files([&fixture]).expect("parse negative fixture with Rust frontend");
        let error = elaborate::elaborate(&spec).expect_err("Rust rejects fixture");

        assert_eq!(rust_error_category(&error.kind), category, "{name}");
        assert_eq!(error.span, span, "{name}");
    }
}
