use std::{path::Path, process::Command};

use p4spec_rust::{
    frontend::parse::parse_files,
    wire::{EL_SCHEMA, Envelope, ocaml::lang::el::SpecCodec},
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

fn export_ocaml_el(repo: &Path, spec_path: &Path) -> Vec<u8> {
    let output = Command::new("opam")
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
        .expect("run pinned OCaml exporter");
    assert!(
        output.status.success(),
        "EL export failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
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
