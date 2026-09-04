//! Command-line integration behavior

use std::{path::Path, process::Command};

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_p4spec-rust"))
}

fn fixture(path: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(path)
}

#[test]
fn test_elab_command_prints_the_intermediate_spec() {
    let output = binary()
        .arg("elab")
        .arg(fixture("cli/simple.watsup"))
        .output()
        .expect("run elab command");

    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "var x : nat\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn test_algo_command_prints_the_algorithmic_spec() {
    let output = binary()
        .arg("algo")
        .arg(fixture("cli/simple.watsup"))
        .output()
        .expect("run algo command");

    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "var x : nat\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn test_algo_command_reports_conversion_errors_on_stderr() {
    let output = binary()
        .arg("algo")
        .arg(fixture("algorithmic/impure_else_premises.watsup"))
        .output()
        .expect("run algo command");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("otherwise branch contains an impure premise")
    );
}

#[test]
fn test_elab_command_reports_frontend_errors_on_stderr() {
    let output = binary()
        .arg("elab")
        .arg(fixture("frontend/negative/malformed-token.watsup"))
        .output()
        .expect("run elab command");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("malformed token")
    );
}

#[test]
fn test_elab_command_reports_elaboration_errors_on_stderr() {
    let output = binary()
        .arg("elab")
        .arg(fixture("elaboration/operator_not_defined.watsup"))
        .output()
        .expect("run elab command");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("operator is not defined")
    );
}

#[test]
fn test_elab_command_requires_at_least_one_path() {
    let output = binary().arg("elab").output().expect("run elab command");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Usage: p4spec-rust <elab|algo> <path>...")
    );
}

#[test]
fn test_help_prints_usage() {
    let output = binary().arg("--help").output().expect("run help command");

    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("Usage: p4spec-rust <elab|algo> <path>...")
    );
    assert!(output.stderr.is_empty());
}
