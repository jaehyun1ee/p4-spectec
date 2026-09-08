use p4spec_rust::{
    interface::p4::{
        error::P4ErrorKind,
        parse::{parse_file, parse_string},
        unparse::P4Unparser,
    },
    lang::data::{
        typ::TypKind,
        value::{Value, ValueArena, ValueKind},
    },
};

fn first_binary<'a>(arena: &'a ValueArena, value: &'a Value) -> Option<&'a Value> {
    if let TypKind::Var(id, _) = arena.typ(value)
        && id.node == "binaryExpression"
    {
        return Some(value);
    }
    match arena.kind(value) {
        ValueKind::Case(case) => case
            .args()
            .into_iter()
            .find_map(|value| first_binary(arena, value)),
        _ => None,
    }
}

fn binary_part<'a>(arena: &'a ValueArena, value: &Value, index: usize) -> &'a Value {
    match arena.kind(value) {
        ValueKind::Case(case) => case.args().into_iter().nth(index).unwrap(),
        _ => panic!("binary expression must be a case value"),
    }
}

fn operator(arena: &ValueArena, value: &Value) -> String {
    P4Unparser::new()
        .render(arena, binary_part(arena, value, 1))
        .unwrap()
}

#[test]
fn test_right_shift_preserves_source_parser_asymmetric_bitwise_binding() {
    let mut arena = ValueArena::new();
    let program_before = parse_string(
        &mut arena,
        "shift.p4",
        "control C() { apply { bit<4> x; x = 4w1 | 4w2 ^ 4w3 & 4w4 >> 4w5; } }",
    )
    .unwrap();
    let shift = first_binary(&arena, &program_before).unwrap();
    assert_eq!(operator(&arena, shift), ">>");
    let bit_or = binary_part(&arena, shift, 0);
    assert_eq!(operator(&arena, bit_or), "|");
    let bit_xor = binary_part(&arena, bit_or, 2);
    assert_eq!(operator(&arena, bit_xor), "^");
    assert_eq!(operator(&arena, binary_part(&arena, bit_xor, 2)), "&");

    let program_after = parse_string(
        &mut arena,
        "shift.p4",
        "control C() { apply { bit<4> x; x = 4w1 >> 4w2 & 4w3 ^ 4w4 | 4w5; } }",
    )
    .unwrap();
    let bit_or = first_binary(&arena, &program_after).unwrap();
    assert_eq!(operator(&arena, bit_or), "|");
    let bit_xor = binary_part(&arena, bit_or, 0);
    assert_eq!(operator(&arena, bit_xor), "^");
    let bit_and = binary_part(&arena, bit_xor, 0);
    assert_eq!(operator(&arena, bit_and), "&");
    assert_eq!(operator(&arena, binary_part(&arena, bit_and, 0)), ">>");
}

#[test]
fn test_right_and_left_shift_share_left_associative_source_precedence() {
    let mut arena = ValueArena::new();
    let program_right_then_left = parse_string(
        &mut arena,
        "shift.p4",
        "control C() { apply { bit<4> x; x = 4w1 >> 4w2 << 4w3; } }",
    )
    .unwrap();
    let left_shift = first_binary(&arena, &program_right_then_left).unwrap();
    assert_eq!(operator(&arena, left_shift), "<<");
    assert_eq!(operator(&arena, binary_part(&arena, left_shift, 0)), ">>");

    let program_left_then_right = parse_string(
        &mut arena,
        "shift.p4",
        "control C() { apply { bit<4> x; x = 4w1 << 4w2 >> 4w3; } }",
    )
    .unwrap();
    let right_shift = first_binary(&arena, &program_left_then_right).unwrap();
    assert_eq!(operator(&arena, right_shift), ">>");
    assert_eq!(operator(&arena, binary_part(&arena, right_shift, 0)), "<<");
}

#[test]
fn test_binary_expression_span_preserves_mapped_token_order() {
    let mut arena = ValueArena::new();
    let program = parse_string(
        &mut arena,
        "preprocessed.p4",
        r#"control C() { apply { bit<4> x; x =
# 200 "later.p4"
4w1
# 10 "earlier.p4"
& 4w2; } }"#,
    )
    .unwrap();
    let binary = first_binary(&arena, &program).unwrap();
    let span = arena.span(binary);

    assert_eq!(span.left.file.as_ref(), "later.p4");
    assert_eq!(span.left.line, 200);
    assert_eq!(span.right.file.as_ref(), "earlier.p4");
    assert_eq!(span.right.line, 10);
}

#[test]
fn test_sized_integer_preserves_child_atom_and_type_label_spans() {
    use p4spec_rust::lang::common::{
        notation::mixfix::Mixfix,
        source::{Position, Span},
    };

    fn find_literal(arena: &ValueArena, value: Value) -> Option<Value> {
        let children = match arena.kind(&value) {
            ValueKind::Case(case) => {
                if let Mixfix::Seq(items) = case
                    && matches!(
                        items.as_slice(),
                        [Mixfix::Arg(_), Mixfix::Atom(_), Mixfix::Arg(_)]
                    )
                    && matches!(arena.typ(&value), TypKind::Var(id, _) if id.node == "integerLiteral")
                {
                    return Some(value);
                }
                case.args()
            }
            ValueKind::List(values) | ValueKind::Tuple(values) => values.iter().collect(),
            ValueKind::Opt(value) => value.iter().collect(),
            _ => Vec::new(),
        };
        children
            .into_iter()
            .find_map(|value| find_literal(arena, *value))
    }

    for (literal, sign) in [("8w3", "W"), ("8s3", "S")] {
        let mut arena = ValueArena::new();
        let prefix = "const bit<8> x = ";
        let program =
            parse_string(&mut arena, "literal.p4", &format!("{prefix}{literal};")).unwrap();
        let value = find_literal(&arena, program).expect("sized integer literal");
        let expected = Span::new(
            Position::new("literal.p4", 1, prefix.len() as i64),
            Position::new("literal.p4", 1, (prefix.len() + literal.len()) as i64),
        );
        assert_eq!(arena.span(&value), &expected);
        let ValueKind::Case(Mixfix::Seq(items)) = arena.kind(&value) else {
            unreachable!()
        };
        let [Mixfix::Arg(width), Mixfix::Atom(atom), Mixfix::Arg(integer)] = items.as_slice()
        else {
            unreachable!()
        };
        assert_eq!(arena.span(width), &expected);
        assert_eq!(arena.span(integer), &expected);
        assert_eq!(arena.location(atom.span), &expected);
        assert_eq!(
            atom.node,
            p4spec_rust::lang::common::notation::atom::Atom::Keyword(sign.to_owned())
        );
        let TypKind::Var(id, _) = arena.typ(&value) else {
            unreachable!()
        };
        assert_eq!(id.span, Span::default());
    }
}

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

/// Draw a single-line progress bar to stderr. libtest only captures the
/// `print!`/`eprint!` macros, so a direct `io::stderr()` write stays visible
/// while the oracle grinds through the p4c corpus (run with `--nocapture`).
fn report_progress(label: &str, done: usize, total: usize) {
    use std::io::Write;
    const WIDTH: usize = 24;
    let filled = (done * WIDTH).checked_div(total).unwrap_or(WIDTH);
    let bar = format!("{}{}", "#".repeat(filled), "-".repeat(WIDTH - filled));
    let mut stderr = std::io::stderr();
    let _ = write!(stderr, "\r{label} [{bar}] {done}/{total}");
    let _ = stderr.flush();
    if done == total {
        let _ = writeln!(stderr);
    }
}

#[test]
fn test_parses_empty_and_declaration_programs() {
    for source in [
        "",
        "const bit<8> width = 8w3;",
        "type bit<8> PortId;",
        "header H { bit<8> field; }",
        "control C() { apply { } }",
        "parser P() { state start { transition accept; } }",
    ] {
        let mut arena = ValueArena::new();
        let program = parse_string(&mut arena, "fixture.p4", source)
            .unwrap_or_else(|error| panic!("failed to parse {source:?}: {error}"));
        assert!(matches!(arena.kind(&program), ValueKind::Case(_)));
        assert_eq!(arena.span(&program).left.file.as_ref(), "fixture.p4");
    }
}

#[test]
fn test_syntax_errors_retain_the_source_location() {
    let mut arena = ValueArena::new();
    let error = parse_string(&mut arena, "broken.p4", "const bit<8> x = ;")
        .expect_err("reject a missing initializer");
    assert_eq!(error.kind, P4ErrorKind::Syntax);
    assert_eq!(error.span.left.file.as_ref(), "broken.p4");
    assert_eq!(error.span.left.line, 1);
}

#[test]
fn test_parses_nested_conditionals_and_switch_fallthrough() {
    let source = r#"
control C() {
    apply {
        bool comparison = a < b > (c);
        if (true) if (false) exit; else exit;
        switch (1) {
            1:
            2: { exit; }
        }
    }
}
"#;
    parse_string(&mut ValueArena::new(), "control.p4", source).expect("parse nested control flow");
}

#[test]
fn test_classifies_names_after_preceding_declarations_reduce() {
    let source = r#"
control Inner() { apply {} }
package Outer(Inner inner);
Outer(Inner()) main;
"#;

    parse_string(&mut ValueArena::new(), "lookahead.p4", source)
        .expect("parse newly declared constructor names");
}

#[test]
fn test_parses_the_positive_p4_corpus() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let corpus = manifest.join("../p4spec/test/micro");
    let includes = [manifest.join("../p4c/p4include")];
    let mut files = Vec::new();
    for directory in [
        "programs",
        "programs-boot",
        "programs-neg",
        "sim-ebpf",
        "sim-psa",
        "sim-v1model",
    ] {
        collect_p4_files(&corpus.join(directory), &mut files);
    }
    files.sort();
    assert!(!files.is_empty(), "the P4 corpus must be present");

    let failures: Vec<_> = files
        .iter()
        .filter_map(|file| {
            parse_file(&mut ValueArena::new(), &includes, file)
                .err()
                .map(|error| format!("{}: {error}", file.display()))
        })
        .collect();
    assert!(
        failures.is_empty(),
        "P4 parse failures:\n{}",
        failures.join("\n")
    );
}

#[test]
fn test_rejects_the_negative_p4_parse_corpus() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let corpus = manifest.join("../p4spec/test/micro/programs-parse-neg");
    let includes = [manifest.join("../p4c/p4include")];
    let mut files = Vec::new();
    collect_p4_files(&corpus, &mut files);
    files.sort();
    assert!(!files.is_empty(), "the negative P4 corpus must be present");

    let accepted: Vec<_> = files
        .iter()
        .filter(|file| parse_file(&mut ValueArena::new(), &includes, file).is_ok())
        .map(|file| file.display().to_string())
        .collect();
    assert!(
        accepted.is_empty(),
        "invalid P4 programs were accepted:\n{}",
        accepted.join("\n")
    );
}

#[test]
fn test_matches_the_positive_parser_oracle() {
    let root = repository_root();
    assert_matches_parser_oracle(
        &root,
        "parser_pos.expected",
        &[
            root.join("p4c/testdata/p4_16_samples"),
            root.join("testdata/custom"),
        ],
    );
}

#[test]
fn test_matches_the_negative_parser_oracle() {
    let root = repository_root();
    assert_matches_parser_oracle(
        &root,
        "parser_neg.expected",
        &[root.join("p4c/testdata/p4_16_errors")],
    );
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("p4spec-rust must be inside the repository")
        .to_path_buf()
}

fn assert_matches_parser_oracle(root: &Path, oracle_name: &str, directories: &[PathBuf]) {
    let oracle_dir = root.join("p4spec/test/parse");
    let oracle = parser_oracle(&oracle_dir, oracle_name);
    let oracle_files = oracle.keys().cloned().collect::<BTreeSet<_>>();

    let mut corpus_files = Vec::new();
    for directory in directories {
        collect_parser_files(directory, &mut corpus_files);
    }
    let corpus_files = corpus_files.into_iter().collect::<BTreeSet<_>>();
    assert_eq!(corpus_files, oracle_files, "P4 parser corpus changed");

    let includes = [root.join("p4c/p4include")];
    let total = oracle.len();
    let mut mismatches = Vec::new();
    for (index, (file, should_parse)) in oracle.into_iter().enumerate() {
        report_progress(&format!("p4 parser {oracle_name}"), index + 1, total);
        let result = parse_file(&mut ValueArena::new(), &includes, &file);
        if result.is_ok() != should_parse {
            mismatches.push(match result {
                Ok(_) => format!("{}: unexpectedly parsed", file.display()),
                Err(error) => format!("{}: {error}", file.display()),
            });
        }
    }
    assert!(
        mismatches.is_empty(),
        "P4 parse expectation mismatches:\n{}",
        mismatches.join("\n")
    );
}

fn parser_oracle(oracle_dir: &Path, oracle_name: &str) -> BTreeMap<PathBuf, bool> {
    let output = fs::read_to_string(oracle_dir.join(oracle_name)).expect("read parser oracle");
    let prefix = ">>> Running parser test on ";
    let mut oracle = output
        .lines()
        .filter_map(|line| line.strip_prefix(prefix))
        .map(|path| {
            let path = oracle_dir.join(path);
            (
                path.canonicalize()
                    .expect("canonicalize parser corpus path"),
                true,
            )
        })
        .collect::<BTreeMap<_, _>>();
    for path in output
        .lines()
        .filter_map(|line| line.strip_prefix("Error parsing file: "))
    {
        let path = oracle_dir
            .join(path)
            .canonicalize()
            .expect("canonicalize rejected parser corpus path");
        *oracle
            .get_mut(&path)
            .expect("rejected file must be in corpus") = false;
    }
    oracle
}

fn collect_parser_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let mut entries = fs::read_dir(directory)
        .expect("read P4 parser corpus directory")
        .collect::<Result<Vec<_>, _>>()
        .expect("read P4 parser corpus entry");
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        if path.is_dir() && entry.file_name() != "include" {
            collect_parser_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "p4") {
            files.push(
                path.canonicalize()
                    .expect("canonicalize parser corpus file"),
            );
        }
    }
}

fn collect_p4_files(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("read P4 corpus directory") {
        let path = entry.expect("read P4 corpus entry").path();
        if path.extension().is_some_and(|extension| extension == "p4") {
            files.push(path);
        }
    }
}

#[test]
fn test_empty_productions_use_previous_token_end_across_whitespace() {
    use p4spec_rust::lang::{
        common::source::Span,
        data::{typ::TypKind, value::Value},
    };
    fn spans(arena: &ValueArena, value: &Value, name: &str, output: &mut Vec<Span>) {
        if let TypKind::Var(id, _) = arena.typ(value)
            && id.node == name
        {
            output.push(arena.span(value).clone());
        }
        if let ValueKind::Case(case) = arena.kind(value) {
            for value in case.args() {
                spans(arena, value, name, output);
            }
        }
    }
    let source = "control C() {\n  action a( \n    bit<8> x) { }\n  apply { }\n}";
    let mut arena = ValueArena::new();
    let value = parse_string(&mut arena, "empty.p4", source).unwrap();
    let mut directions = Vec::new();
    spans(&arena, &value, "direction", &mut directions);
    assert_eq!(directions.len(), 1);
    assert_eq!(
        (directions[0].left.line, directions[0].left.column),
        (2, 11)
    );
    assert_eq!(directions[0].left, directions[0].right);
    let mut annotations = Vec::new();
    spans(&arena, &value, "annotationList", &mut annotations);
    assert!(
        annotations
            .iter()
            .any(|span| span.left.line == 1 && span.left.column == 13)
    );
    assert!(annotations.iter().all(|span| span.left == span.right));
    let mut names = Vec::new();
    spans(&arena, &value, "identifier", &mut names);
    assert!(
        names
            .iter()
            .any(|span| span.left.line == 3 && span.left.column == 11 && span.right.column == 12)
    );
}

#[test]
fn test_initial_empty_production_precedes_whitespace_and_line_directives() {
    use p4spec_rust::lang::{
        common::source::Position,
        data::{typ::TypKind, value::Value},
    };
    fn initial_annotation<'a>(arena: &'a ValueArena, value: &'a Value) -> Option<&'a Value> {
        if let TypKind::Var(id, _) = arena.typ(value)
            && id.node == "annotationList"
        {
            return Some(value);
        }
        match arena.kind(value) {
            ValueKind::Case(case) => case
                .args()
                .into_iter()
                .find_map(|value| initial_annotation(arena, value)),
            _ => None,
        }
    }
    for prefix in ["\n  ", "# 20 \"included.p4\"\n  "] {
        let source = format!("{prefix}control C() {{ apply {{ }} }}");
        let mut arena = ValueArena::new();
        let value = parse_string(&mut arena, "initial.p4", &source).unwrap();
        let annotation = initial_annotation(&arena, &value).unwrap();
        let span = arena.span(annotation);
        assert_eq!(span.left, Position::new("initial.p4", 1, 0));
        assert_eq!(span.right, span.left);
    }
}

#[test]
fn test_syntax_error_after_whitespace_uses_offending_token_span() {
    let source = "\n const bit<8> x =   ;";
    let error = parse_string(&mut ValueArena::new(), "syntax.p4", source).unwrap_err();
    let column = source.lines().nth(1).unwrap().find(';').unwrap() as i64;
    assert_eq!((error.span.left.line, error.span.left.column), (2, column));
    assert_eq!(
        (error.span.right.line, error.span.right.column),
        (2, column + 1)
    );
}
