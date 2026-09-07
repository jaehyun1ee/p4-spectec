use super::*;

#[test]
fn test_composite_al_spec_omits_source_hints_and_extern_relation_inputs() {
    let first = composite_spec("source-a", vec![0]);
    let changed_metadata = composite_spec("source-b", vec![7, 9]);

    assert_eq!(
        Print::to_string(&first),
        Print::to_string(&changed_metadata)
    );
}
