use p4spec_rust::{
    frontend::parse::parse_mixop,
    interface::builtin::maps,
    lang::{
        common::{
            notation::{mixfix::Mixfix, mixop::Mixop},
            source::{Position, Span},
        },
        data::{
            typ,
            value::{get, make},
        },
    },
};

#[test]
fn test_map_update_retains_parsed_punctuation_spans() {
    let span = Span::default();
    let typ_key = typ::make::text();
    let typ_value = typ::make::bool();
    let typ_pair = typ::make::var(super::id("pair"), vec![typ_key.clone(), typ_value.clone()]);
    let typ_pairs = typ::make::list(typ_pair);
    let typ_map = typ::make::var(super::id("map"), vec![typ_key.clone(), typ_value.clone()]);
    let pairs = make::list(&typ_pairs, Vec::new(), span.clone());
    let shape = parse_mixop("`{ k `}").unwrap();
    let map = make::case_(
        &typ_map,
        Mixop::fill(&shape, [pairs]).unwrap(),
        span.clone(),
    );
    let key = make::text("key".to_owned(), span.clone());
    let value = make::bool(true, span);
    let result = maps::add_map(&[typ_key, typ_value], &[map, key, value]).unwrap();
    let Mixfix::Brack(left, inner, right) = get::case(&result).unwrap() else {
        panic!("expected map notation");
    };
    assert_eq!(
        left.span,
        Span::new(Position::new("", 1, 0), Position::new("", 1, 2))
    );
    assert_eq!(
        right.span,
        Span::new(Position::new("", 1, 5), Position::new("", 1, 7))
    );
    let Mixfix::Arg(pairs) = inner.as_ref() else {
        panic!("expected map entries")
    };
    let pairs = get::list(pairs).unwrap();
    let Mixfix::Seq(items) = get::case(&pairs[0]).unwrap() else {
        panic!("expected pair notation")
    };
    let Mixfix::Atom(colon) = &items[1] else {
        panic!("expected colon")
    };
    assert_eq!(
        colon.span,
        Span::new(Position::new("", 1, 2), Position::new("", 1, 5))
    );
}
