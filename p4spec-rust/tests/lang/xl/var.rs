use super::*;

#[test]
fn test_strip_var_suffix_preserves_source_and_all_underscore_suffixes() {
    let source = Span::new(
        Position::new("suffix-source", 0, 0),
        Position::new("suffix-source", 0, 0),
    );
    let suffixed = p4spec_rust::phrase! {
        node: "value_suffix".to_owned(),
        span: source.clone(),
    };
    let apostrophe = p4spec_rust::phrase! {
        node: "value'".to_owned(),
        span: Span::default(),
    };
    let all_underscores = p4spec_rust::phrase! {
        node: "value___".to_owned(),
        span: Span::default(),
    };

    let stripped = var_impl::strip_var_suffix(&suffixed);
    assert_eq!(stripped.node, "value");
    assert_eq!(stripped.span, source);
    assert_eq!(var_impl::strip_var_suffix(&apostrophe).node, "value");
    assert_eq!(
        var_impl::strip_var_suffix(&all_underscores).node,
        "value___"
    );
}
