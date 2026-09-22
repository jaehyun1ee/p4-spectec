//! Public specification transformation entry points
//!
//! Exercises ordered file input, stage errors, and rule-group preservation
//! through the library API used by host applications.

use std::path::{Path, PathBuf};

use p4spec_rust::{Error, lang::traits::print::Print};

fn fixture(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(path)
}

#[test]
fn transformations_preserve_input_order() {
    let paths = [fixture("cli/second.watsup"), fixture("cli/simple.watsup")];
    let outputs = [
        Print::to_string(&p4spec_rust::parse(&paths).unwrap()),
        Print::to_string(&p4spec_rust::elab(&paths).unwrap()),
        Print::to_string(&p4spec_rust::algo(&paths).unwrap()),
        Print::to_string(&p4spec_rust::structure(&paths, true).unwrap()),
        Print::to_string(&p4spec_rust::prosify(&paths).unwrap()),
    ];
    for output in outputs {
        let lines: Vec<_> = output.lines().filter(|line| !line.is_empty()).collect();
        assert_eq!(lines, ["var y : nat", "var x : nat"]);
    }
}

#[test]
fn pipeline_errors_preserve_the_failing_stage_and_location() {
    let path = fixture("frontend/negative/malformed-token.watsup");
    let error = p4spec_rust::prosify([&path]).unwrap_err();
    assert!(matches!(error, Error::Frontend(_)));
    assert!(error.to_string().contains(path.to_str().unwrap()));

    let path = fixture("elaboration/operator_not_defined.watsup");
    let error = p4spec_rust::prosify([&path]).unwrap_err();
    assert!(matches!(error, Error::Elab(_)));
    assert!(error.to_string().contains(path.to_str().unwrap()));

    let path = fixture("algorithmic/impure_else_premises.watsup");
    let error = p4spec_rust::prosify([&path]).unwrap_err();
    assert!(matches!(error, Error::Algo(_)));
    assert!(error.to_string().contains(path.to_str().unwrap()));
}

#[test]
fn structure_exposes_rule_group_preservation() {
    let paths = [fixture("structure/definitions.watsup")];
    for without_rule_groups in [false, true] {
        let spec_sl = p4spec_rust::structure(&paths, without_rule_groups).unwrap();
        assert_eq!(Print::to_string(&spec_sl).contains("Group "), !without_rule_groups);
    }
    // Prose conversion requires the relation's rule groups
    let spec_pl = p4spec_rust::prosify(&paths).unwrap();
    let output = Print::to_string(&spec_pl);
    assert!(output.contains("Group ret:"), "{output}");
    assert!(output.contains("Group else:"), "{output}");
}
