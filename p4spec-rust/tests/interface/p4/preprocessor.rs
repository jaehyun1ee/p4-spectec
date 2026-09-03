use std::{
    fs,
    path::PathBuf,
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use p4spec_rust::interface::p4::preprocessor::preprocess;

fn temporary_file() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("p4spec-{nonce}-{}.p4", process::id()))
}

#[test]
fn test_preprocessing_expands_macros_without_system_headers() {
    let path = temporary_file();
    fs::write(&path, "#define WIDTH 8\nbit<WIDTH> field;\n").unwrap();

    let output = preprocess(&[], &path).unwrap();

    fs::remove_file(&path).unwrap();
    assert!(output.contains("bit<8> field;"));
    assert!(output.contains(path.to_string_lossy().as_ref()));
}

#[test]
fn test_preprocessing_reports_a_typed_failure_for_missing_input() {
    let error = preprocess(&[], "/definitely/missing/p4spec-input.p4").unwrap_err();
    assert!(matches!(
        error.kind,
        p4spec_rust::interface::p4::error::P4ErrorKind::Preprocessor { .. }
    ));
}
