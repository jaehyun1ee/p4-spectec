//! Command-line integration behavior

use std::{fs, path::Path, process::Command};

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_p4spec-rust"))
}

fn fixture(path: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(path)
}

fn assert_spec_matches_expected(stage: &str) {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Rust crate is inside the repository");
    let expected_path = repo.join(format!("p4spec/test/lang/{stage}.expected"));
    let expected = fs::read(&expected_path).expect("read OCaml specification expectation");
    let output = binary()
        .arg(stage)
        .arg(repo.join("spec"))
        .output()
        .expect("run specification command");

    assert!(
        output.status.success(),
        "{stage} failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "unexpected {stage} diagnostic:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    if output.stdout != expected {
        let offset = output
            .stdout
            .iter()
            .zip(&expected)
            .position(|(actual, expected)| actual != expected)
            .unwrap_or(output.stdout.len().min(expected.len()));
        let line = expected[..offset]
            .iter()
            .filter(|&&byte| byte == b'\n')
            .count()
            + 1;
        let actual_path =
            std::env::temp_dir().join(format!("p4spec-rust-{}-{stage}.actual", std::process::id()));
        fs::write(&actual_path, &output.stdout).expect("write actual specification output");
        panic!(
            "{stage} output differs at line {line}; compare {} with {}",
            expected_path.display(),
            actual_path.display()
        );
    }
}

#[test]
fn test_elab_command_matches_expected() {
    assert_spec_matches_expected("elab");
}

#[test]
fn test_algo_command_matches_expected() {
    assert_spec_matches_expected("algo");
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
