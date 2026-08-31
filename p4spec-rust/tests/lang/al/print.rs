use super::*;

#[test]
fn test_composite_al_spec_prints_in_ocaml_order_with_exact_spacing_and_escaping() {
    let spec = composite_spec("source-a", vec![0]);

    assert_eq!(
        Print::to_string(&spec),
        concat!(
            "extern syntax External\n\n",
            "syntax Box<T> = bool\n\n",
            "var state : bool\n\n",
            "extern relation Check: check bool\n\n",
            "relation Evaluate: eval bool => text\n\n",
            "  rulegroup main\n\n",
            "   match\n\n",
            "    (signature) eval signature => %\n",
            "    eval \"line\\n\\\"\\\\\" => %\n",
            "    -- if ready\n\n",
            "   paths\n\n",
            "    rulepath success\n",
            "    -- debug trace\n",
            "    -- output: eval % => \"done\"\n\n",
            "  elsegroup\n\n",
            "  rulegroup fallback_group\n\n",
            "   match\n\n",
            "    (signature) eval fallback_signature => %\n",
            "    eval fallback_input => %\n\n",
            "   paths\n\n",
            "    rulepath fallback\n",
            "    -- output: eval % => \"fallback\"\n\n",
            "relation Ready: ready bool\n\n",
            "  rulegroup ready_group\n\n",
            "   match\n\n",
            "    (signature) ready ready_signature\n",
            "    ready ready_input\n\n",
            "   paths\n\n",
            "    rulepath holds\n",
            "    -- the relation holds\n\n",
            "extern def $external(bool) : bool\n\n",
            "builtin def $builtin(bool) : bool\n\n",
            "tbl def $lookup(bool) : bool =\n",
            "  row 0 :\n",
            "    (signature) table_signature\n",
            "    (key) -> \"row\\tvalue\"\n",
            "    -- if ready\n\n",
            "def $run<T>(bool) : bool =\n\n",
            "  clause 0 : (argument) = \"quoted\\\"\\\\\"\n",
            "  -- if ready\n\n",
            "  clause -1 : (fallback) = false",
        )
    );
}
#[test]
fn test_composite_al_spec_omits_source_hints_and_extern_relation_inputs() {
    let first = composite_spec("source-a", vec![0]);
    let changed_metadata = composite_spec("source-b", vec![7, 9]);

    assert_eq!(
        Print::to_string(&first),
        Print::to_string(&changed_metadata)
    );
}
