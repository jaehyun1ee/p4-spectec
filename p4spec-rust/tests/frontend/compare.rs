use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::Mutex,
};

use p4spec_rust::{
    frontend::{
        error::{FrontendError, LexErrorKind, SyntaxErrorKind},
        parse::parse_files,
    },
    lang::common::source::{Position, Span},
};

static OCAML_EXPORTER: Mutex<()> = Mutex::new(());

/// Draw a single-line progress bar to stderr. libtest only captures the
/// `print!`/`eprint!` macros, so a direct `io::stderr()` write stays visible
/// while these OCaml-differential tests grind through the corpus.
fn report_progress(label: &str, done: usize, total: usize) {
    use std::io::Write;
    const WIDTH: usize = 24;
    let filled = (done * WIDTH).checked_div(total).unwrap_or(WIDTH);
    let bar = format!("{}{}", "#".repeat(filled), "-".repeat(WIDTH - filled));
    let mut stderr = std::io::stderr();
    let _ = write!(stderr, "\r{label} [{bar}] {done}/{total}");
    let _ = stderr.flush();
    if done == total {
        let _ = writeln!(stderr);
    }
}

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

fn position(file: &str, text: &str) -> Position {
    let (line, column) = text
        .split_once('.')
        .expect("OCaml diagnostic position contains line and column");
    Position::new(
        file,
        line.parse::<usize>().expect("decimal line"),
        column.parse::<usize>().expect("decimal column") - 1,
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
    let (col_l, col_r) = range.split_once('-').unwrap_or((range, range));
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
    Diagnostic { kind, span: Span::new(position(file, col_l), position(file, col_r)) }
}

fn rust_diagnostic(error: FrontendError) -> Diagnostic {
    match error {
        FrontendError::Lexical(error) => {
            Diagnostic { kind: DiagnosticKind::Lexical(error.node), span: error.span }
        }
        FrontendError::Syntax(error) => {
            let kind = match error.node {
                SyntaxErrorKind::ExpectedNotationType
                | SyntaxErrorKind::EmptyStructType
                | SyntaxErrorKind::EmptyVariantType
                | SyntaxErrorKind::EmptyType
                | SyntaxErrorKind::HintsInPlainTypeDefinition
                | SyntaxErrorKind::EmptySyntaxDeclaration => DiagnosticKind::Semantic(error.node),
                SyntaxErrorKind::InvalidToken
                | SyntaxErrorKind::UnexpectedEndOfInput
                | SyntaxErrorKind::UnexpectedToken
                | SyntaxErrorKind::ExtraToken => DiagnosticKind::Syntax,
            };
            Diagnostic { kind, span: error.span }
        }
        FrontendError::Io(_) | FrontendError::InvalidUtf8(_) => {
            panic!("negative source fixture must reach lexing or parsing")
        }
    }
}

#[test]
#[ignore = "requires the pinned OCaml toolchain"]
fn test_negative_corpus_matches_ocaml_diagnostics() {
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

    let total = fixtures.len();
    for (index, fixture) in fixtures.into_iter().enumerate() {
        let output = run_ocaml_el(repo, &fixture);
        assert!(!output.status.success(), "OCaml accepted {}", fixture.display());
        let expected = ocaml_diagnostic(&fixture, &output.stderr);
        let actual = parse_files([&fixture])
            .map(|_| panic!("Rust accepted {}", fixture.display()))
            .unwrap_err();
        assert_eq!(
            rust_diagnostic(actual),
            expected,
            "diagnostic changed for {}",
            fixture.display()
        );
        report_progress("frontend negative corpus", index + 1, total);
    }
}
