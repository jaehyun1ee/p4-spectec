use std::{
    path::Path,
    process::{Command, Output},
    sync::Mutex,
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

static OCAML_ORACLE_LOCK: Mutex<()> = Mutex::new(());

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
                let path = format!("{path}.{key}");
                match right.get(key) {
                    Some(right) => first_difference(left, right, &path),
                    None => Some((path, left.to_string(), "<missing>".to_owned())),
                }
            })
        }
        _ => Some((path.to_owned(), left.to_string(), right.to_string())),
    }
}

fn run_ocaml_oracle(repo: &Path, spec_path: &Path) -> Output {
    let _guard = OCAML_ORACLE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
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
    use p4spec_rust::runtime::types::TypeErrorKind;

    match kind {
        ElabErrorKind::Undefined(_) | ElabErrorKind::Type(TypeErrorKind::UndefinedType(_)) => {
            "undefined"
        }
        ElabErrorKind::Duplicate(_) => "duplicate",
        ElabErrorKind::ArityMismatch | ElabErrorKind::Type(TypeErrorKind::ArityMismatch(_)) => {
            "arity_mismatch"
        }
        ElabErrorKind::Type(TypeErrorKind::HigherOrderSubstitution) => "type_error",
        ElabErrorKind::CannotDestructure(_) => "cannot_destructure",
        ElabErrorKind::CannotInfer => "cannot_infer",
        ElabErrorKind::OperatorNotDefined => "operator_not_defined",
        ElabErrorKind::TypeMismatch => "type_mismatch",
        ElabErrorKind::DimensionMismatch => "dimension_mismatch",
        ElabErrorKind::InvalidIteration => "invalid_iteration",
        ElabErrorKind::MisplacedConstruct => "misplaced_construct",
        ElabErrorKind::InvalidIdentifier => "invalid_identifier",
        ElabErrorKind::AmbiguousVariant => "ambiguous_variant",
        ElabErrorKind::InvalidTypeExtension => "invalid_type_extension",
        ElabErrorKind::InvalidCast => "invalid_cast",
        ElabErrorKind::InvalidArgument => "invalid_argument",
        ElabErrorKind::InvalidPremise => "invalid_premise",
        ElabErrorKind::InvalidRule => "invalid_rule",
        ElabErrorKind::InvalidClause => "invalid_clause",
        ElabErrorKind::InvalidDefinition => "invalid_definition",
        ElabErrorKind::InvalidInputHint => "invalid_input_hint",
        ElabErrorKind::AlreadyPopulated => "already_populated",
        ElabErrorKind::NoMatchingAlternative => "no_matching_alternative",
    }
}

#[test]
#[ignore = "requires the pinned OCaml toolchain"]
fn rejected_elaboration_matches_ocaml_category_and_span() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Rust crate is inside the repository");
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/elaboration");

    for (name, ocaml_has_location) in [
        ("ambiguous_variant.watsup", true),
        ("dimension_mismatch.watsup", true),
        ("duplicate_metavariable.watsup", true),
        ("duplicate_type_parameter.watsup", true),
        ("forward_type_parameter_mismatch.watsup", true),
        ("incomparable_subtype.watsup", false),
        ("invalid_cast.watsup", false),
        ("invalid_identifier.watsup", true),
        ("invalid_input_hint.watsup", true),
        ("invalid_iteration.watsup", true),
        ("invalid_rule_identifier.watsup", true),
        ("invalid_type_extension.watsup", true),
        ("operator_not_defined.watsup", false),
        ("non_boolean_table.watsup", true),
        ("unmatched_variant.watsup", false),
        ("undefined_function.watsup", true),
        ("variable_type_collision.watsup", true),
    ] {
        let fixture = fixtures.join(name);
        let output = run_ocaml_oracle(repo, &fixture);
        assert!(!output.status.success(), "OCaml accepted {name}");
        let diagnostic: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "decode OCaml elaboration diagnostic for {name}: {error}\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            )
        });
        let category = diagnostic["category"]
            .as_str()
            .expect("diagnostic category");
        let span = source::decode_region(&diagnostic["span"]).expect("diagnostic span");

        let spec = parse_files([&fixture]).expect("parse negative fixture with Rust frontend");
        let error = elaborate::elaborate(&spec).expect_err("Rust rejects fixture");

        assert_eq!(rust_error_category(&error.kind), category, "{name}");
        if ocaml_has_location {
            assert_eq!(error.span, span, "{name}");
        } else {
            assert_eq!(span, Default::default(), "{name}");
            assert_ne!(error.span, Default::default(), "{name}");
        }
    }
}

#[test]
fn exact_comparison_detects_different_object_keys() {
    let left = serde_json::json!({"left": 1});
    let right = serde_json::json!({"right": 1});

    let difference = first_difference(&left, &right, "payload").expect("different object keys");

    assert_eq!(difference.0, "payload.left");
    assert_eq!(difference.2, "<missing>");
}
