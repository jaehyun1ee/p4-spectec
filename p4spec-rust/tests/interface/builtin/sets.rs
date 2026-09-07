use p4spec_rust::{
    interface::builtin::sets,
    lang::{
        common::{
            notation::mixfix::Mixfix,
            source::{Position, Span},
        },
        data::{
            typ,
            value::{get, make},
        },
    },
};

#[test]
fn test_set_union_retains_parsed_punctuation_spans() {
    let typ_key = typ::make::bool();
    let typ_set = typ::make::var(super::id("set"), vec![typ_key.clone()]);
    let typ_sets = typ::make::list(typ_set);
    let sets = make::list(&typ_sets, Vec::new(), Span::default());
    let result = sets::unions_set(&[typ_key], &[sets]).unwrap();
    let Mixfix::Brack(left, _, right) = get::case(&result).unwrap() else {
        panic!("expected set notation");
    };
    assert_eq!(
        left.span,
        Span::new(Position::new("", 1, 0), Position::new("", 1, 2))
    );
    assert_eq!(
        right.span,
        Span::new(Position::new("", 1, 5), Position::new("", 1, 7))
    );
}
