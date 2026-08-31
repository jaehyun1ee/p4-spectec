use p4spec_rust::{
    interface::p4::{
        error::P4ErrorKind,
        parse::{parse_file, parse_string},
    },
    runtime::value::ValueKind,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[test]
fn parses_empty_and_declaration_programs() {
    for source in [
        "",
        "const bit<8> width = 8w3;",
        "header H { bit<8> field; }",
        "control C() { apply { } }",
        "parser P() { state start { transition accept; } }",
    ] {
        let program = parse_string("fixture.p4", source)
            .unwrap_or_else(|error| panic!("failed to parse {source:?}: {error}"));
        assert!(matches!(program.kind, ValueKind::Case(_)));
        assert_eq!(program.span.left.file.as_ref(), "fixture.p4");
    }
}

#[test]
fn syntax_errors_retain_the_source_location() {
    let error =
        parse_string("broken.p4", "const bit<8> x = ;").expect_err("reject a missing initializer");
    assert_eq!(error.kind, P4ErrorKind::Syntax);
    assert_eq!(error.span.left.file.as_ref(), "broken.p4");
    assert_eq!(error.span.left.line, 1);
}

#[test]
fn parses_nested_conditionals_and_switch_fallthrough() {
    let source = r#"
control C() {
    apply {
        if (true) if (false) exit; else exit;
        switch (1) {
            1:
            2: { exit; }
        }
    }
}
"#;
    parse_string("control.p4", source).expect("parse nested control flow");
}

#[test]
fn parses_the_positive_p4_corpus() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let corpus = manifest.join("../p4spec/test/micro");
    let includes = [manifest.join("../../../p4c/p4include")];
    let mut files = Vec::new();
    for directory in [
        "programs",
        "programs-boot",
        "programs-neg",
        "sim-ebpf",
        "sim-psa",
        "sim-v1model",
    ] {
        collect_p4_files(&corpus.join(directory), &mut files);
    }
    files.sort();
    assert!(!files.is_empty(), "the P4 corpus must be present");

    let failures: Vec<_> = files
        .iter()
        .filter_map(|file| {
            parse_file(&includes, file)
                .err()
                .map(|error| format!("{}: {error}", file.display()))
        })
        .collect();
    assert!(
        failures.is_empty(),
        "P4 parse failures:\n{}",
        failures.join("\n")
    );
}

fn collect_p4_files(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("read P4 corpus directory") {
        let path = entry.expect("read P4 corpus entry").path();
        if path.extension().is_some_and(|extension| extension == "p4") {
            files.push(path);
        }
    }
}
