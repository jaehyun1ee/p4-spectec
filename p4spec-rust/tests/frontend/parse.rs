use std::{
    fs,
    path::{Path, PathBuf},
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};

use p4spec_rust::{
    frontend::parse::{parse_files, parse_mixop},
    lang::{
        common::{
            notation::mixfix::Mixfix,
            source::{Position, Span},
        },
        el::ast::{DefKind, ExpKind},
    },
};

static TEMP_DIRECTORY_ID: AtomicUsize = AtomicUsize::new(0);

#[test]
fn test_parses_runtime_mixop_shapes() {
    let mixop = parse_mixop("name '=' expression").expect("parse mixop shape");
    assert_eq!(mixop.args().len(), 2);
}

#[test]
fn test_runtime_mixop_punctuation_preserves_string_source_positions() {
    let Mixfix::Seq(items) = parse_mixop("k ':' v").unwrap() else {
        panic!("expected pair notation");
    };
    let Mixfix::Atom(colon) = &items[1] else {
        panic!("expected colon");
    };
    assert_eq!(colon.span, Span::new(Position::new("", 1, 2), Position::new("", 1, 5)));

    let Mixfix::Brack(atom_l, _, atom_r) = parse_mixop("`{ k `}").unwrap() else {
        panic!("expected bracket notation");
    };
    assert_eq!(atom_l.span, Span::new(Position::new("", 1, 0), Position::new("", 1, 2)));
    assert_eq!(atom_r.span, Span::new(Position::new("", 1, 5), Position::new("", 1, 7)));
}

struct TempDirectory {
    path: PathBuf,
}

impl TempDirectory {
    fn new() -> Self {
        let id = TEMP_DIRECTORY_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir()
            .join(format!("p4spec-rust-frontend-parse-{}-{id}", std::process::id()));
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
fn test_empty_trailing_syntax_does_not_extend_a_definition_span() {
    let spec = crate::spec_fixture::parse("var b : bool\n\nvar i : int")
        .expect("parse adjacent definitions");

    assert_eq!((spec[0].span.right.line, spec[0].span.right.column), (1, 12));
}

#[test]
fn test_trailing_hint_sets_the_definition_span() {
    let source = concat!(
        "dec $ite<X>(bool, X, X) : X\n",
        "  hint(prose_in %1 \"if\" %0 \"otherwise\" %2)\n",
        "def $ite<X>(true, X_t, X_f) = X_t",
    );
    let spec = crate::spec_fixture::parse(source).expect("parse a declaration with a hint");

    assert_eq!((spec[0].span.right.line, spec[0].span.right.column), (2, 42));
}

#[test]
fn test_bracketed_type_syntax_sets_the_definition_span() {
    let spec = crate::spec_fixture::parse("syntax set<K> = `{ K* `}")
        .expect("parse a bracketed notation type");

    assert_eq!((spec[0].span.right.line, spec[0].span.right.column), (1, 24));
}

#[test]
fn test_parse_file_uses_the_path_in_source_locations() {
    let directory = TempDirectory::new();
    let path = directory.path("one.watsup");
    fs::write(&path, "var one : nat").expect("write SpecTec file");

    let spec = parse_files([&path]).expect("parse SpecTec file");

    assert_eq!(spec[0].span.left.file.as_ref(), path.to_string_lossy());
    assert!(Rc::ptr_eq(&spec[0].span.left.file, &spec[0].span.right.file));
    let span = spec[0].span.clone();
    assert!(Rc::ptr_eq(&spec[0].span.left.file, &span.left.file));
    assert!(matches!(&spec[0].node, DefKind::Var(def) if def.id.node == "one"));
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
        .map(|def| match &def.node {
            DefKind::Var(def) => def.id.node.as_str(),
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
        DefKind::FuncDef(def)
            if matches!(&def.exp.node, ExpKind::Id(id) if id.node == "X")
    ));
}

#[test]
fn test_parse_file_reports_invalid_utf8_at_the_invalid_byte() {
    let directory = TempDirectory::new();
    let path = directory.path("invalid.watsup");
    fs::write(&path, b"var x : nat\n\xff").expect("write invalid UTF-8 file");

    let error = parse_files([&path]).expect_err("reject invalid UTF-8");
    assert_eq!(error.code.as_deref(), Some("parse/source-encoding-invalid"));

    assert_eq!(error.labels[0].span.left, Position::new(path.to_string_lossy(), 2, 0));
    assert_eq!(error.labels[0].span.right, Position::new(path.to_string_lossy(), 2, 1));
}

#[test]
fn test_parse_file_reports_io_and_syntax_failures_with_file_spans() {
    let directory = TempDirectory::new();
    let missing = directory.path("missing.watsup");
    let error = parse_files([&missing]).expect_err("report missing file");
    assert_eq!(error.code.as_deref(), Some("parse/file-read-failed"));
    assert_eq!(error.labels[0].span.left, Position::new(missing.to_string_lossy(), 0, 0));

    let invalid = directory.path("syntax.watsup");
    fs::write(&invalid, "def").expect("write invalid SpecTec file");
    let error = parse_files([&invalid]).expect_err("report syntax error");
    assert_eq!(error.code.as_deref(), Some("parse/input-incomplete"));
    assert_eq!(error.labels[0].span.left, Position::new(invalid.to_string_lossy(), 1, 3));
    assert_eq!(error.labels[0].span.right, Position::new(invalid.to_string_lossy(), 1, 3));
}

#[test]
fn test_parse_bytes_distinguishes_nested_comments_from_comment_text() {
    use p4spec_rust::frontend::parse::parse_utf8_bytes;
    use std::rc::Rc;

    let cases: &[(&[u8], &str, usize, usize)] = &[
        (b"(; outer (; inner ;)\n\xff", "parse/comment-encoding-invalid", 2, 0),
        (b"(; closed ;)\xff", "parse/source-encoding-invalid", 1, 12),
        (b";; (; line comment\n\xff", "parse/source-encoding-invalid", 2, 0),
        (b"\"(;\"\xff", "parse/source-encoding-invalid", 1, 4),
        (b"(; \xc3\xa9\n\xff", "parse/comment-encoding-invalid", 2, 0),
    ];
    for (bytes, code, line, column) in cases {
        let report = parse_utf8_bytes(Rc::from("bytes.watsup"), bytes).unwrap_err();
        assert_eq!(report.code.as_deref(), Some(*code));
        assert_eq!(report.labels[0].span.left, Position::new("bytes.watsup", *line, *column));
        assert_eq!(report.labels[0].span.right, Position::new("bytes.watsup", *line, column + 1));
    }
}

#[test]
fn test_parse_bytes_uses_source_encoding_fallback_after_lexical_errors() {
    use p4spec_rust::frontend::parse::parse_utf8_bytes;

    for (bytes, column) in [(&b"@ (;\xff"[..], 4), (&b"\"\\q\" (;\xff"[..], 7)] {
        let report = parse_utf8_bytes(Rc::from("bytes.watsup"), bytes).unwrap_err();
        assert_eq!(report.code.as_deref(), Some("parse/source-encoding-invalid"));
        assert_eq!(report.labels[0].span.left, Position::new("bytes.watsup", 1, column));
        assert_eq!(report.labels[0].span.right, Position::new("bytes.watsup", 1, column + 1));
        assert!(report.labels[0].message.contains("0xFF"));
    }
}

#[test]
fn test_missing_path_fails_before_parsing_collected_files() {
    let directory = TempDirectory::new();
    let invalid = directory.path("invalid.watsup");
    let missing = directory.path("missing.watsup");
    fs::write(&invalid, "}").unwrap();

    let report = parse_files([&invalid, &missing]).unwrap_err();
    assert_eq!(report.code.as_deref(), Some("parse/file-read-failed"));
    assert_eq!(report.labels[0].span.left, Position::new(missing.to_string_lossy(), 0, 0));
}

#[test]
fn test_parser_diagnostics_preserve_actual_and_expected_tokens() {
    use p4spec_rust::frontend::parse::parse_text;

    let report = parse_text(Rc::from("syntax.watsup"), "var x :").unwrap_err();
    let text = report.labels[0].message.as_str();
    assert!(text.contains("expected"), "{text}");
    assert!(text.contains("nat"), "{text}");
    assert!(text.contains("identifier"), "{text}");
    assert!(!text.contains("UPID") && !text.contains("NL2"), "{text}");
    for (source, actual) in [("var x : }", "}"), ("var : nat", ":"), ("var x : 123", "123")] {
        let report = parse_text(Rc::from("syntax.watsup"), source).unwrap_err();
        assert!(report.message.contains(actual), "{}", report.message);
        assert!(report.labels[0].message.contains("expected"));
    }
}

#[test]
fn test_source_utf8_errors_identify_invalid_and_truncated_bytes() {
    use p4spec_rust::frontend::parse::parse_utf8_bytes;

    for (bytes, code, hex, truncated) in [
        (&b"\xff"[..], "parse/source-encoding-invalid", "0xFF", false),
        (&b"(; \xe2\x82"[..], "parse/comment-encoding-invalid", "0xE2 0x82", true),
    ] {
        let report = parse_utf8_bytes(Rc::from("bytes.watsup"), bytes).unwrap_err();
        assert_eq!(report.code.as_deref(), Some(code));
        assert!(report.labels[0].message.contains(hex));
        assert_eq!(report.labels[0].message.contains("truncated"), truncated);
    }
}

#[test]
fn test_missing_file_diagnostic_names_path_and_cause() {
    let directory = TempDirectory::new();
    let path = directory.path("missing.watsup");
    let report = parse_files([&path]).unwrap_err();
    assert!(report.message.contains("cannot read"));
    assert!(report.message.contains("missing.watsup"));
    assert!(report.message.contains("file does not exist"));
    assert!(!report.message.contains("entity"));
}

#[test]
fn test_unexpected_token_spelling_preserves_payload_on_later_lines() {
    use p4spec_rust::frontend::parse::parse_text;

    for (source, actual) in [
        ("var x : nat\n\nvar 0xFE : nat", "0xFE"),
        ("var x : nat\n\nvar \"é\\n\" : nat", "é"),
        ("var x : nat\n\nvar bad( : nat", "bad("),
    ] {
        let report = parse_text(Rc::from("syntax.watsup"), source).unwrap_err();
        assert!(report.message.contains(actual), "{}", report.message);
        assert!(!report.message.contains('\n'));
        assert_eq!(report.labels[0].span.left.line, 3);
    }
}

#[test]
fn test_unexpected_layout_tokens_keep_their_identity_with_empty_spans() {
    use p4spec_rust::frontend::parse::parse_text;

    for (source, actual) in [
        ("var x :\n\nvar y : nat", "blank line"),
        ("var x :\n\n\nvar y : nat", "two blank lines"),
        ("var x :\n| var y : nat", "newline followed by `|`"),
    ] {
        let report = parse_text(Rc::from("layout.watsup"), source).unwrap_err();
        assert_eq!(report.code.as_deref(), Some("parse/token-invalid"));
        assert!(report.message.contains(actual), "{}", report.message);
        assert_eq!(report.message, format!("unexpected {actual}"));
        assert!(report.labels[0].message.contains("expected"));
    }
}

#[test]
fn test_expected_identifiers_include_contextually_bound_uppercase_names() {
    use p4spec_rust::frontend::parse::parse_text;

    parse_text(Rc::from("bindings.watsup"), "var X : nat\n\nvar y : X")
        .expect("bound uppercase names are accepted as identifiers");
    for source in ["var : nat", "var X : nat\n\nvar y : }"] {
        let report = parse_text(Rc::from("bindings.watsup"), source).unwrap_err();
        assert!(report.labels[0].message.contains("an identifier"));
        assert!(!report.labels[0].message.contains("lowercase"));
    }
}

#[test]
fn test_plain_type_hints_are_rejected_in_both_definition_grammar_branches() {
    for source in ["syntax foo = nat hint(blah)", "syntax foo = | nat hint(blah)"] {
        let report = crate::spec_fixture::parse(source).unwrap_err();
        assert_eq!(report.code.as_deref(), Some("parse/plain-type-hint-unsupported"), "{source}");
    }
}
