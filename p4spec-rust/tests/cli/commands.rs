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

fn repo() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned()
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

    assert_eq!(output.status.code(), Some(1));
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

    assert_eq!(output.status.code(), Some(1));
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

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("operator is not defined")
    );
}

#[test]
fn test_commands_require_at_least_one_path() {
    for command in ["elab", "algo"] {
        let output = binary().arg(command).output().expect("run command");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("Usage:"));
        assert!(stderr.contains("<PATH>"));
    }
}

#[test]
fn test_help_prints_commands() {
    for flag in ["-h", "--help"] {
        let output = binary().arg(flag).output().expect("run help");
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("Commands:"));
        assert!(stdout.contains("elab"));
        assert!(stdout.contains("algo"));
    }
}

#[test]
fn test_subcommand_help_prints_paths_without_processing_inputs() {
    for command in ["elab", "algo"] {
        for flag in ["-h", "--help"] {
            let output = binary()
                .args([command, "missing.watsup", flag])
                .output()
                .expect("run command help");
            assert!(output.status.success());
            assert!(output.stderr.is_empty());
            let stdout = String::from_utf8(output.stdout).unwrap();
            assert!(stdout.contains("Usage:"));
            assert!(stdout.contains("<PATH>"));
        }
    }
}

#[test]
fn test_version_prints_package_version() {
    let output = binary().arg("--version").output().expect("run version");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("p4spec-rust {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn test_invalid_arguments_report_usage_errors() {
    for args in [
        vec![],
        vec!["unknown"],
        vec!["--unknown"],
        vec!["elab", "--unknown"],
        vec!["algo", "--unknown"],
    ] {
        let output = binary().args(args).output().expect("run invalid arguments");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr).unwrap().contains("Usage:"));
    }
}

#[test]
fn test_commands_preserve_multiple_input_order() {
    for command in ["elab", "algo"] {
        let output = binary()
            .arg(command)
            .arg(fixture("cli/second.watsup"))
            .arg(fixture("cli/simple.watsup"))
            .output()
            .expect("run multiple inputs");
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            "var y : nat\n\nvar x : nat\n"
        );
    }
}

#[test]
fn test_commands_accept_hyphenated_paths_after_separator() {
    for command in ["elab", "algo"] {
        let output = binary()
            .current_dir(fixture("cli"))
            .args([command, "--", "-input.watsup"])
            .output()
            .expect("run hyphenated input");
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        assert_eq!(String::from_utf8(output.stdout).unwrap(), "var z : nat\n");
    }
}

fn run_command(relation: &str, program: &str) -> Command {
    let mut command = binary();
    command
        .args(["run", "--al"])
        .arg(fixture("cli/run/types.watsup"))
        .arg(fixture("cli/run/relations.watsup"))
        .args(["--rel", relation, "-p"])
        .arg(fixture(program));
    command
}

#[test]
fn test_run_al_native_success_and_multiple_spec_paths() {
    let output = run_command("Pass", "cli/run/empty.p4").output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"passed\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn test_run_al_initializes_placeholder_extern_objects() {
    let repo = repo();
    let output = binary()
        .args(["run", "--al"])
        .arg(repo.join("spec"))
        .args(["--rel", "Program_inst", "-p"])
        .arg(repo.join("p4c/testdata/p4_16_samples/action_profile-bmv2.p4"))
        .arg("-i")
        .arg(repo.join("p4c/p4include"))
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"passed\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn test_run_al_distinguishes_syntax_and_runtime_failures() {
    for (relation, program, category) in [
        ("Pass", "cli/run/invalid.p4", "syntax error:"),
        ("Reject", "cli/run/empty.p4", "runtime error:"),
    ] {
        let output = run_command(relation, program).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.starts_with(category), "{error}");
        if category == "syntax error:" {
            assert!(error.contains("invalid.p4"), "{error}");
        } else {
            assert!(error.contains("Reject"), "{error}");
        }
    }
}

#[test]
fn test_run_al_repeated_include_directories() {
    let output = run_command("Pass", "cli/run/includes.p4")
        .arg("-i")
        .arg(fixture("cli/run/first"))
        .arg("-i")
        .arg(fixture("cli/run/second"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"passed\n");
}

#[test]
fn test_run_al_det_and_guard_controls_change_execution() {
    for (relation, flag) in [("Ambiguous", "--det"), ("Unchecked", "--guard")] {
        let output = run_command(relation, "cli/run/empty.p4").output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = run_command(relation, "cli/run/empty.p4")
            .arg(flag)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).starts_with("runtime error:"));
    }
    let output = run_command("Pass", "cli/run/empty.p4")
        .args(["--det", "--guard"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_run_al_requires_flags_and_rejects_unsupported_options() {
    for args in [
        vec!["run"],
        vec!["run", "--al"],
        vec!["run", "--al", "spec", "--rel", "Pass"],
        vec!["run", "--al", "spec", "-p", "empty.p4"],
        vec!["run", "spec", "--rel", "Pass", "-p", "empty.p4"],
        vec!["run", "--al", "--rel", "Pass", "-p", "empty.p4"],
        vec!["run", "--sl"],
    ] {
        let output = binary().args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains("Usage:"));
    }
    for flag in [
        "--no-cache",
        "--trace",
        "--profile",
        "--unknown",
        "-i",
        "--rel",
        "-p",
    ] {
        let output = run_command("Pass", "cli/run/empty.p4")
            .arg(flag)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{flag}");
    }
}

#[test]
fn test_run_al_reports_spec_load_failure() {
    let output = binary()
        .args(["run", "--al"])
        .arg(fixture("frontend/negative/malformed-token.watsup"))
        .args(["--rel", "Pass", "-p"])
        .arg(fixture("cli/run/empty.p4"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("malformed token"));
}

#[test]
fn test_run_help_lists_only_implemented_controls() {
    let output = binary().args(["run", "--help"]).output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let usage = String::from_utf8(output.stdout).unwrap();
    for flag in ["--al", "--rel", "--det", "--guard"] {
        assert!(usage.contains(flag));
    }
    for flag in ["--no-cache", "--trace", "--profile"] {
        assert!(!usage.contains(flag));
    }
}
