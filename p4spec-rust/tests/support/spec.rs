use p4spec_rust::{diagnostic::Report, frontend::parse::parse_files, lang::el::ast::Spec};
use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_FILE: AtomicUsize = AtomicUsize::new(0);

pub fn parse(source: &str) -> Result<Spec, Box<Report>> {
    let id = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("p4spec-test-{}-{id}.watsup", std::process::id()));
    fs::write(&path, source).expect("write specification fixture");
    let result = parse_files([&path]);
    fs::remove_file(path).expect("remove specification fixture");
    result
}
