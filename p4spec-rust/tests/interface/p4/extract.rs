use p4spec_rust::{
    interface::p4::{extract, parse::parse_string},
    lang::il::ast::TypKind,
    runtime::value::{Value, ValueKind},
};

#[test]
fn test_extracts_declaration_names_and_type_parameters() {
    let program = parse_string(
        "extract.p4",
        "header Header<T> { T field; } const bit<8> width = 8w3;",
    )
    .expect("parse declarations");
    let header = find_typed(&program, "headerTypeDeclaration").expect("find header declaration");
    assert_eq!(extract::declaration_id(header).unwrap(), "Header");
    assert!(extract::declaration_has_type_parameters(header).unwrap());

    let parameters = find_typed(&program, "typeParameterListOpt").expect("find type parameters");
    assert_eq!(extract::type_parameter_ids(parameters).unwrap(), ["T"]);

    let constant = find_typed(&program, "constantDeclaration").expect("find constant");
    assert_eq!(extract::declaration_id(constant).unwrap(), "width");
}

#[test]
fn test_extraction_rejects_unrelated_values() {
    let program = parse_string("empty.p4", "").expect("parse empty program");
    assert_eq!(
        extract::declaration_id(&program),
        Err(extract::ExtractError::Declaration)
    );
    assert_eq!(
        extract::type_id_of_ref(&program),
        Err(extract::ExtractError::TypeReference)
    );
}

fn find_typed<'a>(value: &'a Value, name: &str) -> Option<&'a Value> {
    if matches!(&value.note, TypKind::Var(id, _) if id.node == name) {
        return Some(value);
    }
    match &value.node {
        ValueKind::Struct(fields) => fields.iter().find_map(|(_, value)| find_typed(value, name)),
        ValueKind::Case(value_case) => value_case
            .split()
            .1
            .into_iter()
            .find_map(|value| find_typed(value, name)),
        ValueKind::Tuple(values) | ValueKind::List(values) => {
            values.iter().find_map(|value| find_typed(value, name))
        }
        ValueKind::Opt(Some(value)) => find_typed(value, name),
        _ => None,
    }
}
