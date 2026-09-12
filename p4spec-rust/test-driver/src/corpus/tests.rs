use super::*;

#[test]
fn test_expected_preserves_paths_and_negative_success() {
    let records =
        parse_expected("pass\tp4c/a/same.p4\nfail\tp4c/b/same.p4\nexclude\tp4c/c.p4\n").unwrap();
    assert_eq!(records[Path::new("p4c/a/same.p4")], Outcome::Pass);
    assert_eq!(records[Path::new("p4c/b/same.p4")], Outcome::Fail);
    assert_eq!(records[Path::new("p4c/c.p4")], Outcome::Exclude);
}

#[test]
fn test_expected_rejects_duplicate_invalid_and_escaping_records() {
    for text in [
        "pass\ta.p4\npass\ta.p4\n",
        "pass\ta.p4\nfail\ta.p4\n",
        "unknown\ta.p4\n",
        "fail\t../a.p4\n",
        "pass\t/a.p4\n",
        "pass\ta/./b.p4\n",
        "pass\ta//b.p4\n",
        "",
        "pass\ta.p4\textra\n",
    ] {
        assert!(parse_expected(text).is_err(), "{text:?}");
    }
}

#[test]
fn test_file_comparison_detects_swapped_results_and_incomplete_runs() {
    let expected = parse_expected("pass\ta.p4\nfail\tb.p4\nexclude\tc.p4\n").unwrap();
    let mut results = Results::new(&expected);
    assert!(!results.record(Path::new("a.p4"), Outcome::Fail).unwrap());
    assert!(!results.record(Path::new("b.p4"), Outcome::Pass).unwrap());
    assert!(results.finish().is_err());
    assert!(results.record(Path::new("c.p4"), Outcome::Exclude).unwrap());
    assert_eq!(results.mismatched, 2);
    assert!(results.finish().is_err());
    assert!(results.record(Path::new("a.p4"), Outcome::Pass).is_err());
    assert!(
        results
            .record(Path::new("extra.p4"), Outcome::Pass)
            .is_err()
    );
}

#[test]
fn test_collection_applies_exact_excludes_and_skips_include_directories() {
    let dir = std::env::temp_dir().join(format!("p4spec-corpus-{}", std::process::id()));
    fs::create_dir_all(dir.join("programs/include")).unwrap();
    fs::create_dir_all(dir.join("programs/sub")).unwrap();
    fs::create_dir_all(dir.join("excludes")).unwrap();
    for path in [
        "programs/a.p4",
        "programs/sub/a.p4",
        "programs/include/skipped.p4",
    ] {
        fs::write(dir.join(path), "").unwrap();
    }
    fs::write(
        dir.join("excludes/static.exclude"),
        "#comment\nprograms/a.p4\n programs/sub/a.p4\n\n",
    )
    .unwrap();
    let paths = collect(&dir.join("programs"), ".p4").unwrap();
    assert_eq!(
        paths,
        vec![dir.join("programs/a.p4"), dir.join("programs/sub/a.p4")]
    );
    let excludes = collect_excludes(&dir.join("excludes")).unwrap();
    assert!(excludes.contains("programs/a.p4"));
    assert!(!excludes.contains("programs/sub/a.p4"));
    assert!(excludes.contains(" programs/sub/a.p4"));
    assert!(excludes.contains(""));
    assert!(collect(&dir.join("missing"), ".p4").is_err());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn test_inventory_rejects_missing_extra_and_changed_exclusions() {
    let expected = parse_expected("pass\ta.p4\nexclude\tb.p4\n").unwrap();
    let paths = vec![PathBuf::from("a.p4"), PathBuf::from("b.p4")];
    let excludes = BTreeSet::from(["b.p4".to_owned()]);
    validate_inventory(&expected, &paths, &excludes).unwrap();
    assert!(validate_inventory(&expected, &paths[..1], &excludes).is_err());
    assert!(
        validate_inventory(
            &expected,
            &[paths.clone(), vec![PathBuf::from("c.p4")]].concat(),
            &excludes
        )
        .is_err()
    );
    assert!(
        validate_inventory(
            &expected,
            &[paths.clone(), vec![PathBuf::from("a.p4")]].concat(),
            &excludes
        )
        .is_err()
    );
    assert!(validate_inventory(&expected, &paths, &BTreeSet::new()).is_err());
}
