use std::{
    path::Path,
    process::{Command, Output},
    sync::Mutex,
};

use p4spec_rust::{
    frontend::parse::{parse_files, parse_string},
    pass::{
        algo::{self, AlgoErrorKind},
        elaborate,
    },
};

static OCAML_EXPORT_LOCK: Mutex<()> = Mutex::new(());

fn run_ocaml_exporter(repo: &Path, spec_path: &Path) -> Output {
    let _guard = OCAML_EXPORT_LOCK
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
            "./p4spec/bin/main.exe",
            "--",
            "export-json",
            "-stage",
            "al",
            spec_path.to_str().expect("UTF-8 specification path"),
        ])
        .current_dir(repo)
        .output()
        .expect("run pinned OCaml AL exporter")
}

fn rust_error_category(kind: &AlgoErrorKind) -> &'static str {
    match kind {
        AlgoErrorKind::UndefinedType => "undefined_type",
        AlgoErrorKind::InconsistentDimensions => "inconsistent_dimensions",
        AlgoErrorKind::NonInvertibleBinding(_) => "non_invertible_binding",
        AlgoErrorKind::EmptyIteration => "empty_iteration",
        AlgoErrorKind::UndeterminedBindingDimension => "undetermined_binding_dimension",
        AlgoErrorKind::PatternArityMismatch { .. } => "pattern_arity_mismatch",
        AlgoErrorKind::TypeArgumentArityMismatch { .. } => "type_argument_arity_mismatch",
        AlgoErrorKind::Type(_) => "type_error",
        AlgoErrorKind::AntiUnification => "anti_unification",
        AlgoErrorKind::ExpressionArityMismatch { .. } => "expression_arity_mismatch",
        AlgoErrorKind::InputHint(_) => "input_hint",
        AlgoErrorKind::FreeBindings => "free_bindings",
        AlgoErrorKind::BindingsNotShallow => "bindings_not_shallow",
        AlgoErrorKind::ShallowSideConditions => "shallow_side_conditions",
        AlgoErrorKind::BindingOnBothEqualitySides => "binding_on_both_equality_sides",
        AlgoErrorKind::UnexpectedIterationBindings => "unexpected_iteration_bindings",
        AlgoErrorKind::ImpureElsePremises => "impure_else_premises",
        AlgoErrorKind::NonVariantPatternType => "non_variant_pattern_type",
        AlgoErrorKind::InvalidTablePattern => "invalid_table_pattern",
        AlgoErrorKind::InvalidTableParameter => "invalid_table_parameter",
        AlgoErrorKind::OverlappingTablePatterns => "overlapping_table_patterns",
        AlgoErrorKind::MissingTablePatterns => "missing_table_patterns",
    }
}

fn ocaml_error_category(message: &str) -> Option<&'static str> {
    message
        .contains("non-pure premises alongside an otherwise premise")
        .then_some("impure_else_premises")
}

#[test]
#[ignore = "requires the pinned OCaml toolchain"]
fn test_rejected_conversion_matches_ocaml_category_and_span() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Rust crate is inside the repository");
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/algorithmic");

    let name = "impure_else_premises.watsup";
    let fixture = fixtures.join(name);
    let output = run_ocaml_exporter(repo, &fixture);
    assert!(!output.status.success(), "OCaml accepted {name}");
    let stderr = std::str::from_utf8(&output.stderr).expect("UTF-8 OCaml diagnostic");
    let diagnostic = stderr.lines().next_back().expect("OCaml diagnostic line");
    let (span, message) = diagnostic
        .split_once(": ")
        .expect("located OCaml diagnostic");

    let spec_el = parse_files([&fixture]).expect("parse negative fixture with Rust frontend");
    let spec_il = elaborate::elaborate(spec_el).expect("elaborate negative fixture with Rust");
    let error = algo::convert(spec_il).expect_err("Rust rejects fixture");

    assert_eq!(
        Some(rust_error_category(&error.kind)),
        ocaml_error_category(message),
        "{name}"
    );
    assert_eq!(error.span.to_string(), span, "{name}");
}

#[test]
fn test_conversion_rejects_overlapping_crossed_alias_table_rows() {
    let source = r#"
syntax typeIR
syntax typeId = text
syntax typedefTypeIR = TYPEDEF typeId typeIR
syntax intTypeIR = INT
syntax typeIR =
  | intTypeIR
  | typedefTypeIR

tbl dec $compat(typeIR, typeIR) : bool
tbl def $compat =
  | (INT, INT) => true
  | (TYPEDEF _ typeIR_l, typeIR_r) => true
  | (typeIR_l, TYPEDEF _ typeIR_r) => true
  | (_, _) => false
"#;
    let spec_el = parse_string(source).expect("parse crossed alias table");
    let spec_il = elaborate::elaborate(spec_el).expect("elaborate crossed alias table");

    let error = algo::convert(spec_il).expect_err("crossed alias rows overlap by syntax");

    assert_eq!(error.kind, AlgoErrorKind::OverlappingTablePatterns);
}
