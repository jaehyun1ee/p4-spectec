use p4spec_rust::{
    diagnostic::{LabelStyle, ReportKind},
    frontend::parse::parse_text,
    pass::elaborate,
};

#[test]
fn test_dimension_conflict_labels_the_later_minimum_as_primary() {
    let spec_el = parse_text(
        "dimension.watsup".into(),
        "dec $same<K>(K**, K?) : bool\ndef $same<K>(K_x**, K_x?) = true\n",
    )
    .unwrap();
    let report = elaborate::convert(spec_el).unwrap_err();
    let ReportKind::Cause(diagnostic) = &report.kind else { panic!("expected dimension cause") };
    assert_eq!(diagnostic.code.as_deref(), Some("elab/iteration-dimension-mismatch"));
    assert_eq!(diagnostic.labels[0].style, LabelStyle::Primary);
    assert_eq!(diagnostic.labels[0].span.left.line, 2);
    assert_eq!(diagnostic.labels[0].span.left.column, 20);
    assert_eq!(diagnostic.labels[1].style, LabelStyle::Secondary);
    assert_eq!(diagnostic.labels[1].span.left.column, 13);
    assert!(diagnostic.message.contains("`K**` and `K?`"), "{}", diagnostic.message);
}
