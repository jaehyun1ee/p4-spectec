use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::Mutex,
};

use p4spec_rust::{
    frontend::{
        error::{FrontendError, LexErrorKind, SyntaxErrorKind},
        parse::{parse_file, parse_files},
    },
    lang::common::source::{Position, Span},
    wire::{EL_SCHEMA, Envelope, ocaml::lang::el::SpecCodec},
};
use serde_json::Value;

static OCAML_EXPORTER: Mutex<()> = Mutex::new(());

#[derive(Debug, PartialEq, Eq)]
enum DiagnosticKind {
    Lexical(LexErrorKind),
    Syntax,
    Semantic(SyntaxErrorKind),
}

#[derive(Debug, PartialEq, Eq)]
struct Diagnostic {
    kind: DiagnosticKind,
    span: Span,
}

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

fn run_ocaml_el(repo: &Path, spec_path: &Path) -> Output {
    let _guard = OCAML_EXPORTER.lock().expect("OCaml exporter lock");
    Command::new("opam")
        .args([
            "exec",
            "--",
            "dune",
            "exec",
            "--root",
            repo.to_str().expect("UTF-8 repository path"),
            "./p4spec/bin/main.exe",
            "--",
            "export-json",
            "-stage",
            "el",
            spec_path.to_str().expect("UTF-8 specification path"),
        ])
        .current_dir(repo)
        .output()
        .expect("run pinned OCaml exporter")
}

fn export_ocaml_el(repo: &Path, spec_path: &Path) -> Vec<u8> {
    let output = run_ocaml_el(repo, spec_path);
    assert!(
        output.status.success(),
        "EL export failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn position(file: &str, text: &str) -> Position {
    let (line, column) = text
        .split_once('.')
        .expect("OCaml diagnostic position contains line and column");
    Position::new(
        file,
        line.parse::<i64>().expect("decimal line"),
        column.parse::<i64>().expect("decimal column") - 1,
    )
}

fn ocaml_diagnostic(path: &Path, stderr: &[u8]) -> Diagnostic {
    let file = path.to_str().expect("UTF-8 fixture path");
    let output = String::from_utf8_lossy(stderr);
    let diagnostic = output
        .trim()
        .strip_prefix(file)
        .and_then(|diagnostic| diagnostic.strip_prefix(':'))
        .unwrap_or_else(|| panic!("OCaml diagnostic starts with the fixture path: {output}"));
    let (range, message) = diagnostic
        .split_once(": ")
        .expect("OCaml diagnostic contains a source range and message");
    let (left, right) = range.split_once('-').unwrap_or((range, range));
    let kind = match message {
        "unclosed text literal" => DiagnosticKind::Lexical(LexErrorKind::UnclosedTextLiteral),
        "illegal escape" => DiagnosticKind::Lexical(LexErrorKind::IllegalEscape),
        "unclosed comment" => DiagnosticKind::Lexical(LexErrorKind::UnclosedComment),
        "hex literal out of range" => DiagnosticKind::Lexical(LexErrorKind::HoleNumberOutOfRange),
        "malformed token" => DiagnosticKind::Lexical(LexErrorKind::MalformedToken),
        "misplaced unicode character" => {
            DiagnosticKind::Lexical(LexErrorKind::MisplacedUnicodeCharacter)
        }
        "syntax error: unexpected token" => DiagnosticKind::Syntax,
        "expected notation type" => DiagnosticKind::Semantic(SyntaxErrorKind::ExpectedNotationType),
        "empty struct type" => DiagnosticKind::Semantic(SyntaxErrorKind::EmptyStructType),
        "empty variant type" => DiagnosticKind::Semantic(SyntaxErrorKind::EmptyVariantType),
        "empty type" => DiagnosticKind::Semantic(SyntaxErrorKind::EmptyType),
        "hints not allowed in plain type definition" => {
            DiagnosticKind::Semantic(SyntaxErrorKind::HintsInPlainTypeDefinition)
        }
        "empty syntax declaration" => {
            DiagnosticKind::Semantic(SyntaxErrorKind::EmptySyntaxDeclaration)
        }
        message => panic!("unmapped OCaml diagnostic category: {message}"),
    };
    Diagnostic {
        kind,
        span: Span::new(position(file, left), position(file, right)),
    }
}

fn rust_diagnostic(error: FrontendError) -> Diagnostic {
    match error {
        FrontendError::Lexical(error) => Diagnostic {
            kind: DiagnosticKind::Lexical(error.kind),
            span: error.span,
        },
        FrontendError::Syntax(error) => {
            let kind = match error.kind {
                SyntaxErrorKind::ExpectedNotationType
                | SyntaxErrorKind::EmptyStructType
                | SyntaxErrorKind::EmptyVariantType
                | SyntaxErrorKind::EmptyType
                | SyntaxErrorKind::HintsInPlainTypeDefinition
                | SyntaxErrorKind::EmptySyntaxDeclaration => DiagnosticKind::Semantic(error.kind),
                SyntaxErrorKind::InvalidToken
                | SyntaxErrorKind::UnexpectedEndOfInput
                | SyntaxErrorKind::UnexpectedToken
                | SyntaxErrorKind::ExtraToken => DiagnosticKind::Syntax,
            };
            Diagnostic {
                kind,
                span: error.span,
            }
        }
        FrontendError::Io { .. } | FrontendError::InvalidUtf8 { .. } => {
            panic!("negative source fixture must reach lexing or parsing")
        }
    }
}

#[test]
#[ignore = "requires the pinned OCaml toolchain"]
fn positive_corpus_matches_ocaml_el_exactly() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Rust crate is inside the repository");
    let spec_path = repo.join("spec");

    let document = export_ocaml_el(repo, &spec_path);
    let envelope = Envelope::<Value>::from_slice(&document).expect("decode EL envelope");
    assert_eq!(envelope.schema(), EL_SCHEMA);
    assert_eq!(envelope.kind(), "el");
    let expected = SpecCodec::decode(&envelope.into_payload()).expect("decode OCaml EL AST");

    let actual = parse_files([&spec_path]).expect("parse positive corpus with Rust frontend");
    assert_eq!(actual.len(), expected.len(), "definition count changed");
    for (index, (actual_definition, expected_definition)) in
        actual.iter().zip(&expected).enumerate()
    {
        if actual_definition != expected_definition {
            let actual_value = SpecCodec::encode(&vec![actual_definition.clone()])
                .expect("encode Rust definition");
            let expected_value = SpecCodec::encode(&vec![expected_definition.clone()])
                .expect("encode OCaml definition");
            let (path, actual_value, expected_value) =
                first_difference(&actual_value, &expected_value, "definition")
                    .expect("unequal definitions have a differing JSON value");
            panic!(
                "definition {index} at {:?} changed at {path}:\nRust: {actual_value}\nOCaml: {expected_value}",
                actual_definition.span,
            );
        }
    }
    assert_eq!(actual, expected);
}

#[test]
#[ignore = "requires the pinned OCaml toolchain"]
fn negative_corpus_matches_ocaml_diagnostics() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Rust crate is inside the repository");
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/frontend/negative");
    let mut fixtures = fs::read_dir(corpus)
        .expect("read negative frontend corpus")
        .map(|entry| entry.expect("read negative fixture entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "watsup")
        })
        .collect::<Vec<_>>();
    fixtures.sort();
    assert!(!fixtures.is_empty(), "negative frontend corpus is empty");

    for fixture in fixtures {
        let output = run_ocaml_el(repo, &fixture);
        assert!(
            !output.status.success(),
            "OCaml accepted {}",
            fixture.display()
        );
        let expected = ocaml_diagnostic(&fixture, &output.stderr);
        let actual = parse_file(&fixture)
            .map(|_| panic!("Rust accepted {}", fixture.display()))
            .unwrap_err();
        assert_eq!(
            rust_diagnostic(actual),
            expected,
            "diagnostic changed for {}",
            fixture.display()
        );
    }
}
