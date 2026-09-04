use p4spec_rust::lang::traits::print::Print;
use p4spec_rust::stf::{
    ast::{Action, Condition, CounterKind, IdOrIndex, MatchKind, Statement},
    compare, parse, print, transform,
};
use std::path::{Path, PathBuf};

#[test]
fn test_parses_commands_in_source_order() {
    let source = r#"
        packet 1 001122aB
        expect 2 0011**ab$
        add ingress.ipv4_lpm 10 hdr.ipv4.dstAddr:0x0a000001/24 set_nhop(port:3) = entry0
        check_counter c_packets(4) packets >= 0x10
        wait
    "#;

    let statements = parse::parse_str("commands.stf", source).expect("valid STF");
    assert_eq!(statements.len(), 5);
    assert_eq!(
        statements[0].node,
        Statement::Packet("1".into(), "001122aB".into())
    );
    assert_eq!(
        statements[1].node,
        Statement::Expect("2".into(), Some("0011**ab".into()), true)
    );
    assert_eq!(
        statements[2].node,
        Statement::Add {
            table: "ingress.ipv4_lpm".into(),
            priority: Some(10),
            matches: vec![(
                "hdr.ipv4.dstAddr".into(),
                MatchKind::Slash("0x0a000001".into(), "24".into()),
            )],
            action: Action {
                name: "set_nhop".into(),
                args: vec![("port".into(), "3".into())],
            },
            id: Some("entry0".into()),
        }
    );
    assert_eq!(
        statements[3].node,
        Statement::CheckCounter {
            id: "c_packets".into(),
            target: IdOrIndex::Index("4".into()),
            check: (Some(CounterKind::Packets), Condition::Ge, "0x10".into()),
        }
    );
    assert!(
        statements
            .windows(2)
            .all(|pair| pair[0].span.left <= pair[1].span.left)
    );
    assert_eq!(statements[0].span.left.line, 2);
}

#[test]
fn test_parses_packet_wildcards_and_comments() {
    let source = "# generated\nexpect 0 0a**??ff\nno_packet\n";
    let statements = parse::parse_str("wildcards.stf", source).expect("valid STF");
    assert_eq!(
        statements[0].node,
        Statement::Expect("0".into(), Some("0a**ff".into()), false)
    );
    assert_eq!(statements[1].node, Statement::NoPacket);
}

#[test]
fn test_reports_filename_line_and_column() {
    let error = parse::parse_str("bad.stf", "packet port nope\n").unwrap_err();
    let rendered = error.to_string();
    assert!(rendered.contains("bad.stf:1."), "{rendered}");
}

#[test]
fn test_rejects_priorities_outside_the_ocaml_integer_range() {
    let source = "add table 4611686018427387904 field:1 action()\n";

    let error = parse::parse_str("priority.stf", source).expect_err("priority overflow");

    assert!(matches!(
        error.kind,
        p4spec_rust::stf::error::StfErrorKind::InvalidPriority(_)
    ));
}

#[test]
fn test_rejects_digits_outside_the_selected_radix() {
    for number in ["0b102", "12b", "0x0g"] {
        let source = format!("register_read r {number}\n");
        let error = parse::parse_str("number.stf", &source).expect_err(number);
        assert_eq!(error.span.left.file.as_ref(), "number.stf");
        assert_eq!(error.span.left.line, 1);
    }
}

#[test]
fn test_transforms_names_matches_and_actions() {
    assert_eq!(
        transform::rewrite_name_prefix("pipe0.tbl", &["pipe"], "ingress"),
        "ingress.tbl"
    );
    assert_eq!(
        transform::replace_name_substrings("foo.MyIngress.bar", &["myingress"], "ingress"),
        "foo.ingress.bar"
    );
    assert_eq!(
        transform::rewrite_valid_match("hdr.$valid$"),
        "hdr.isValid()"
    );
    assert_eq!(
        transform::unqualify_action("ingress.ipv4.set_port"),
        "set_port"
    );
}

#[test]
fn test_compares_wildcard_packets_and_prints_statements() {
    assert!(compare::packet_matches("a01f", "a**f"));
    assert!(!compare::packet_matches("a01f", "a*f"));

    let statement = Statement::SetDefault {
        table: "ingress.tbl".into(),
        action: Action {
            name: "drop".into(),
            args: vec![],
        },
    };
    assert_eq!(
        print::statement(&statement),
        "setdefault \"ingress.tbl\" \"drop\"()"
    );
    assert_eq!(
        print::convert_dollar_to_brackets("hdr.$12.field"),
        "hdr.[12].field"
    );
    assert_eq!(Print::to_string(&statement), print::statement(&statement));

    let program = parse::parse_str("print.stf", "wait\nno_packet\n").unwrap();
    assert_eq!(Print::to_string(&program), "wait\nno_packet");
}

#[test]
fn test_parses_repository_stf_corpus() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut files = Vec::new();
    collect_stf(&repository.join("p4spec/test"), &mut files);
    collect_stf(&repository.join("testdata"), &mut files);
    files.sort();
    assert!(!files.is_empty());

    let mut failures = Vec::new();
    for file in &files {
        if let Err(error) = parse::parse_file(file) {
            failures.push(format!("{}: {error}", file.display()));
        }
    }
    assert!(
        failures.is_empty(),
        "failed to parse {} of {} STF files:\n{}",
        failures.len(),
        files.len(),
        failures.join("\n")
    );
}

fn collect_stf(directory: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_stf(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "stf") {
            files.push(path);
        }
    }
}
