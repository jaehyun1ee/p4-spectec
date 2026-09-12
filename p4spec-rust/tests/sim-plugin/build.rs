use p4spec_rust::{
    interp::al::Config,
    sim_plugin::build::{BuildError, build},
};

#[test]
fn test_unsupported_architecture_precedes_spec_loading() {
    assert!(
        matches!(build(vec![], "unknown", Config::new(true, false, false)), Err(BuildError::UnsupportedArchitecture(name)) if name == "unknown")
    );
}

#[test]
fn test_selected_architecture_uses_its_program_entry() {
    let path = std::env::temp_dir().join(format!("p4spec-stf-build-{}.p4", std::process::id()));
    std::fs::write(&path, "").unwrap();
    for (arch, relation) in [
        ("ebpf", "EBPF_init"),
        ("psa", "PSA_init"),
        ("v1model", "V1Model_init"),
    ] {
        let mut simulator = build(vec![], arch, Config::new(true, false, false)).unwrap();
        let error = match simulator.init_pipe(&[], &path) {
            Ok(_) => panic!("an empty specification cannot initialize {arch}"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            p4spec_rust::sim_plugin::runner::Error::Runtime(_)
        ));
        assert!(error.to_string().contains(relation), "{error}");
    }
    std::fs::remove_file(path).unwrap();
}
