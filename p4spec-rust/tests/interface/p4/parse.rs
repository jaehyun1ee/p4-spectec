use p4spec_rust::{
    interface::p4::{
        error::P4ErrorKind,
        parse::{parse_file, parse_string},
    },
    lang::data::value::ValueKind,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[test]
fn test_parses_empty_and_declaration_programs() {
    for source in [
        "",
        "const bit<8> width = 8w3;",
        "header H { bit<8> field; }",
        "control C() { apply { } }",
        "parser P() { state start { transition accept; } }",
    ] {
        let program = parse_string("fixture.p4", source)
            .unwrap_or_else(|error| panic!("failed to parse {source:?}: {error}"));
        assert!(matches!(program.node, ValueKind::Case(_)));
        assert_eq!(program.span.left.file.as_ref(), "fixture.p4");
    }
}

#[test]
fn test_syntax_errors_retain_the_source_location() {
    let error =
        parse_string("broken.p4", "const bit<8> x = ;").expect_err("reject a missing initializer");
    assert_eq!(error.kind, P4ErrorKind::Syntax);
    assert_eq!(error.span.left.file.as_ref(), "broken.p4");
    assert_eq!(error.span.left.line, 1);
}

#[test]
fn test_parses_nested_conditionals_and_switch_fallthrough() {
    let source = r#"
control C() {
    apply {
        bool comparison = a < b > (c);
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
fn test_classifies_names_after_preceding_declarations_reduce() {
    let source = r#"
control Inner() { apply {} }
package Outer(Inner inner);
Outer(Inner()) main;
"#;

    parse_string("lookahead.p4", source).expect("parse newly declared constructor names");
}

#[test]
fn test_parses_the_positive_p4_corpus() {
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

#[test]
fn test_rejects_the_negative_p4_parse_corpus() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let corpus = manifest.join("../p4spec/test/micro/programs-parse-neg");
    let includes = [manifest.join("../../../p4c/p4include")];
    let mut files = Vec::new();
    collect_p4_files(&corpus, &mut files);
    files.sort();
    assert!(!files.is_empty(), "the negative P4 corpus must be present");

    let accepted: Vec<_> = files
        .iter()
        .filter(|file| parse_file(&includes, file).is_ok())
        .map(|file| file.display().to_string())
        .collect();
    assert!(
        accepted.is_empty(),
        "invalid P4 programs were accepted:\n{}",
        accepted.join("\n")
    );
}

#[test]
fn test_matches_the_positive_parser_oracle() {
    let root = repository_root();
    assert_matches_parser_oracle(
        &root,
        "parser_pos.expected",
        &[
            root.join("p4c/testdata/p4_16_samples"),
            root.join("testdata/custom"),
        ],
    );
}

#[test]
fn test_matches_the_negative_parser_oracle() {
    let root = repository_root();
    assert_matches_parser_oracle(
        &root,
        "parser_neg.expected",
        &[root.join("p4c/testdata/p4_16_errors")],
    );
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("p4spec-rust must be inside the repository")
        .to_path_buf()
}

fn assert_matches_parser_oracle(root: &Path, oracle_name: &str, directories: &[PathBuf]) {
    let oracle_dir = root.join("p4spec/test/parse");
    let oracle = parser_oracle(&oracle_dir, oracle_name);
    let oracle_files = oracle.keys().cloned().collect::<BTreeSet<_>>();

    let mut corpus_files = Vec::new();
    for directory in directories {
        collect_parser_files(directory, &mut corpus_files);
    }
    let corpus_files = corpus_files.into_iter().collect::<BTreeSet<_>>();
    assert_eq!(corpus_files, oracle_files, "P4 parser corpus changed");

    let includes = [root.join("p4c/p4include")];
    let mismatches = oracle
        .into_iter()
        .filter_map(|(file, should_parse)| {
            let result = parse_file(&includes, &file);
            (result.is_ok() != should_parse).then(|| match result {
                Ok(_) => format!("{}: unexpectedly parsed", file.display()),
                Err(error) => format!("{}: {error}", file.display()),
            })
        })
        .collect::<Vec<_>>();
    assert!(
        mismatches.is_empty(),
        "P4 parse expectation mismatches:\n{}",
        mismatches.join("\n")
    );
}

fn parser_oracle(oracle_dir: &Path, oracle_name: &str) -> BTreeMap<PathBuf, bool> {
    let output = fs::read_to_string(oracle_dir.join(oracle_name)).expect("read parser oracle");
    let prefix = ">>> Running parser test on ";
    let mut oracle = output
        .lines()
        .filter_map(|line| line.strip_prefix(prefix))
        .map(|path| {
            let path = oracle_dir.join(path);
            (
                path.canonicalize()
                    .expect("canonicalize parser corpus path"),
                true,
            )
        })
        .collect::<BTreeMap<_, _>>();
    for path in output
        .lines()
        .filter_map(|line| line.strip_prefix("Error parsing file: "))
    {
        let path = oracle_dir
            .join(path)
            .canonicalize()
            .expect("canonicalize rejected parser corpus path");
        *oracle
            .get_mut(&path)
            .expect("rejected file must be in corpus") = false;
    }
    oracle
}

fn collect_parser_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let mut entries = fs::read_dir(directory)
        .expect("read P4 parser corpus directory")
        .collect::<Result<Vec<_>, _>>()
        .expect("read P4 parser corpus entry");
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        if path.is_dir() && entry.file_name() != "include" {
            collect_parser_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "p4") {
            files.push(
                path.canonicalize()
                    .expect("canonicalize parser corpus file"),
            );
        }
    }
}

fn collect_p4_files(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("read P4 corpus directory") {
        let path = entry.expect("read P4 corpus entry").path();
        if path.extension().is_some_and(|extension| extension == "p4") {
            files.push(path);
        }
    }
}
