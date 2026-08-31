use p4spec_rust::{
    frontend::parse::parse_string,
    pass::{algo, elaborate},
};

#[test]
fn test_conversion_accepts_crossed_alias_table_rows_from_source() {
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
    let spec_el = parse_string(source).expect("parse crossed alias table");
    let spec_il = elaborate::elaborate(&spec_el).expect("elaborate crossed alias table");

    algo::convert(&spec_il).expect("pinned conversion accepts source-distinct alias rows");
}
