use std::{
    fs,
    path::{Path, PathBuf},
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};

use p4spec_rust::{
    frontend::{
        error::{FrontendError, SyntaxErrorKind},
        parse::{parse_file, parse_files, parse_string},
    },
    lang::{
        common::source::Position,
        el::ast::{DefKind, ExpKind},
    },
};

static TEMP_DIRECTORY_ID: AtomicUsize = AtomicUsize::new(0);

struct TempDirectory {
    path: PathBuf,
}

impl TempDirectory {
    fn new() -> Self {
        let id = TEMP_DIRECTORY_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "p4spec-rust-frontend-parse-{}-{id}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create isolated test directory");
        Self { path }
    }

    fn path(&self, relative: impl AsRef<Path>) -> PathBuf {
        self.path.join(relative)
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.path).expect("remove isolated test directory");
    }
}

#[test]
fn test_parse_string_returns_el_with_an_empty_source_name() {
    let spec = parse_string("var x : nat").expect("parse SpecTec string");

    assert!(matches!(&spec[0].node, DefKind::Var(definition) if definition.id.node == "x"));
    assert_eq!(spec[0].span.left, Position::new("", 1, 0));
}

#[test]
fn test_empty_trailing_syntax_does_not_extend_a_definition_span() {
    let spec = parse_string("var b : bool\n\nvar i : int").expect("parse adjacent definitions");

    assert_eq!(spec[0].span.right, Position::new("", 1, 12));
}

#[test]
fn test_trailing_hint_sets_the_definition_span() {
    let source = concat!(
        "dec $ite<X>(bool, X, X) : X\n",
        "  hint(prose_in %1 \"if\" %0 \"otherwise\" %2)\n",
        "def $ite<X>(true, X_t, X_f) = X_t",
    );
    let spec = parse_string(source).expect("parse a declaration with a hint");

    assert_eq!(spec[0].span.right, Position::new("", 2, 42));
}

#[test]
fn test_bracketed_type_syntax_sets_the_definition_span() {
    let spec = parse_string("syntax set<K> = `{ K* `}").expect("parse a bracketed notation type");

    assert_eq!(spec[0].span.right, Position::new("", 1, 24));
}

#[test]
fn test_parse_file_uses_the_path_in_source_locations() {
    let directory = TempDirectory::new();
    let path = directory.path("one.watsup");
    fs::write(&path, "var one : nat").expect("write SpecTec file");

    let spec = parse_file(&path).expect("parse SpecTec file");

    assert_eq!(spec[0].span.left.file.as_ref(), path.to_string_lossy());
    assert!(Rc::ptr_eq(
        &spec[0].span.left.file,
        &spec[0].span.right.file
    ));
    let span = spec[0].span.clone();
    assert!(Rc::ptr_eq(&spec[0].span.left.file, &span.left.file));
    assert!(matches!(&spec[0].node, DefKind::Var(definition) if definition.id.node == "one"));
}

#[test]
fn test_parse_files_preserves_path_order_and_expands_directories_in_name_order() {
    let directory = TempDirectory::new();
    let first = directory.path("first.watsup");
    let specs = directory.path("specs");
    let include = specs.join("include");
    let nested = specs.join("nested");
    fs::create_dir(&specs).expect("create specs directory");
    fs::create_dir(&include).expect("create ignored include directory");
    fs::create_dir(&nested).expect("create nested specs directory");
    fs::write(&first, "var first : nat").expect("write first SpecTec file");
    fs::write(specs.join("b.watsup"), "var b : nat").expect("write b SpecTec file");
    fs::write(specs.join("a.watsup"), "var a : nat").expect("write a SpecTec file");
    fs::write(specs.join("ignored.txt"), "var text : nat").expect("write non-SpecTec file");
    fs::write(include.join("hidden.watsup"), "var hidden : nat")
        .expect("write ignored SpecTec file");
    fs::write(nested.join("c.watsup"), "var c : nat").expect("write nested SpecTec file");

    let spec = parse_files([first.as_path(), specs.as_path()]).expect("parse SpecTec paths");
    let ids = spec
        .iter()
        .map(|definition| match &definition.node {
            DefKind::Var(definition) => definition.id.node.as_str(),
            _ => panic!("expected variable definition"),
        })
        .collect::<Vec<_>>();

    assert_eq!(ids, ["first", "a", "b", "c"]);
}

#[test]
fn test_parse_files_shares_uppercase_variable_context_between_files() {
    let directory = TempDirectory::new();
    let binding = directory.path("binding.watsup");
    let use_site = directory.path("use.watsup");
    fs::write(&binding, "var X : nat").expect("write binding SpecTec file");
    fs::write(&use_site, "def $use() = X").expect("write use-site SpecTec file");

    let spec = parse_files([binding, use_site]).expect("parse related SpecTec files");

    assert!(matches!(
        &spec[1].node,
        DefKind::FuncDef(definition)
            if matches!(&definition.exp.node, ExpKind::Var(id) if id.node == "X")
    ));
}

#[test]
fn test_parse_file_reports_invalid_utf8_at_the_invalid_byte() {
    let directory = TempDirectory::new();
    let path = directory.path("invalid.watsup");
    fs::write(&path, b"var x : nat\n\xff").expect("write invalid UTF-8 file");

    let error = parse_file(&path).expect_err("reject invalid UTF-8");
    let FrontendError::InvalidUtf8(error) = error else {
        panic!("expected invalid UTF-8 error")
    };

    assert_eq!(error.span.left, Position::new(path.to_string_lossy(), 2, 0));
    assert_eq!(
        error.span.right,
        Position::new(path.to_string_lossy(), 2, 1)
    );
}

#[test]
fn test_parse_file_reports_io_and_syntax_failures_with_file_spans() {
    let directory = TempDirectory::new();
    let missing = directory.path("missing.watsup");
    let FrontendError::Io(error) = parse_file(&missing).expect_err("report missing file") else {
        panic!("expected I/O error")
    };
    assert_eq!(
        error.span.left,
        Position::new(missing.to_string_lossy(), 0, 0)
    );

    let invalid = directory.path("syntax.watsup");
    fs::write(&invalid, "def").expect("write invalid SpecTec file");
    let FrontendError::Syntax(error) = parse_file(&invalid).expect_err("report syntax error")
    else {
        panic!("expected syntax error")
    };
    assert_eq!(error.node, SyntaxErrorKind::UnexpectedToken);
    assert_eq!(
        error.span.left,
        Position::new(invalid.to_string_lossy(), 1, 3)
    );
    assert_eq!(
        error.span.right,
        Position::new(invalid.to_string_lossy(), 1, 3)
    );
}
