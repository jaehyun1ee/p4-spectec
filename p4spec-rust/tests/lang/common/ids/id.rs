use p4spec_rust::lang::common::{
    ids::id::strip_suffix,
    source::{Position, Span},
};

#[test]
fn test_strip_suffix_preserves_source_and_all_underscore_suffixes() {
    let source =
        Span::new(Position::new("suffix-source", 0, 0), Position::new("suffix-source", 0, 0));
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

    let stripped = suffixed.strip_suffix();
    assert_eq!(stripped.node, "value");
    assert_eq!(strip_suffix(&suffixed.node), stripped.node);
    assert_eq!(stripped.span, source);
    assert_eq!(apostrophe.strip_suffix().node, "value");
    assert_eq!(strip_suffix(&apostrophe.node), "value");
    assert_eq!(all_underscores.strip_suffix().node, "value___");
    assert_eq!(strip_suffix(&all_underscores.node), "value___");
}
