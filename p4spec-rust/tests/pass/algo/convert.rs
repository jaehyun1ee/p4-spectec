use p4spec_rust::pass::{
    algo::{self, AlgoErrorKind},
    elaborate,
};

#[test]
fn test_conversion_rejects_overlapping_crossed_alias_table_rows() {
    let source = r#"
syntax typeIR
syntax typeId = text
syntax typedefTypeIR = TYPEDEF typeId typeIR
syntax intTypeIR = INT
syntax typeIR =
  | intTypeIR
  | typedefTypeIR

tbl dec $compat(typeIR, typeIR) : bool
tbl def $compat =
  | (INT, INT) => true
  | (TYPEDEF _ typeIR_l, typeIR_r) => true
  | (typeIR_l, TYPEDEF _ typeIR_r) => true
  | (_, _) => false
"#;
    let spec_el = crate::spec_fixture::parse(source).expect("parse crossed alias table");
    let spec_il = elaborate::convert(spec_el).expect("elaborate crossed alias table");

    let error = algo::convert(spec_il).expect_err("crossed alias rows overlap by syntax");

    assert_eq!(error.kind, AlgoErrorKind::OverlappingTablePatterns);
}
