use std::rc::Rc;

use super::{
    arena::Arena,
    notation::Mixfix,
    typ,
    value::{
        ValueFlat,
        external::{self, Encoding},
        get, make,
    },
};
use crate::lang::common::{
    notation::atom::Atom,
    source::{Position, Span},
};

fn span(line: usize) -> Span {
    Span::new(Position::new("fixture", line, 0), Position::new("fixture", line, 1))
}

fn sample(arena: &mut Arena) -> ValueFlat {
    let typ = Rc::new(
        typ::make::var(crate::phrase!(node: "Fixture".to_owned(), span: span(1)), vec![]).node,
    );
    let value_bool = make::bool(arena, true, span(2)).unwrap();
    let value_text = make::text(arena, "argument".to_owned(), span(3)).unwrap();
    let value_inner = make::case(
        arena,
        typ.clone(),
        Mixfix::seq(vec![
            Mixfix::atom(crate::phrase!(node: Atom::Keyword("INNER".to_owned()), span: span(4))),
            Mixfix::arg(value_bool),
            Mixfix::arg(value_text),
        ]),
        span(5),
    )
    .unwrap();
    let value_opt = make::opt(
        arena,
        Rc::new(typ::make::opt(typ::make::bool()).node),
        Some(value_bool),
        span(6),
    )
    .unwrap();
    let value_list = make::list(
        arena,
        Rc::new(typ::make::list(typ::make::text()).node),
        vec![value_text],
        span(7),
    )
    .unwrap();
    let value_empty = make::case(arena, typ.clone(), Mixfix::seq(vec![]), span(8)).unwrap();
    make::case(
        arena,
        typ,
        Mixfix::brack(
            crate::phrase!(node: Atom::LParen, span: span(9)),
            Mixfix::seq(vec![
                Mixfix::arg(value_inner),
                Mixfix::arg(value_opt),
                Mixfix::arg(value_list),
                Mixfix::arg(value_empty),
            ]),
            crate::phrase!(node: Atom::RParen, span: span(10)),
        ),
        span(11),
    )
    .unwrap()
}

fn assert_fixture(name: &str, json: &serde_json::Value) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/lang/data/fixtures")
        .join(name);
    let json_expect: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(*json, json_expect);
}

#[test]
fn value_json_preserves_annotations_and_case_topology() {
    let mut arena = Arena::new();
    let value = sample(&mut arena);
    let json = external::encode(&arena, &value).unwrap();
    assert_fixture("value-independent.json", &json);
    let mut arena_other = Arena::new();
    make::text(&mut arena_other, "unrelated".to_owned(), Span::default()).unwrap();
    let value_other: ValueFlat =
        external::decode_with(&mut arena_other, Encoding::ArenaIndependent, &json).unwrap();
    assert_eq!(external::encode(&arena_other, &value_other).unwrap(), json);
    let json_relative = external::encode_with(&arena, Encoding::ArenaRelative, &value).unwrap();
    assert_fixture("value-relative.json", &json_relative);
    let value_again: ValueFlat =
        external::decode_with(&mut arena, Encoding::ArenaRelative, &json_relative).unwrap();
    assert_eq!(value_again, value);
    assert_fixture(
        "case-relative.json",
        &external::encode_with(&arena, Encoding::ArenaRelative, get::case(&arena, &value).unwrap())
            .unwrap(),
    );
}
