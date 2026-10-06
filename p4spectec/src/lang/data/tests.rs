use std::rc::Rc;

use super::{
    arena::Arena,
    encoding::Encoding,
    notation::Mixfix,
    typ,
    value::{
        ValueFlat,
        external::{self},
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

#[test]
fn notation_preserves_exact_spans_and_canonical_identity() {
    use super::notation::{MixopArena, MixopTree};
    let mut arena_mixop = MixopArena::new();
    let atom_a = crate::phrase!(node: Atom::Keyword("K".to_owned()), span: span(1));
    let atom_b = crate::phrase!(node: Atom::Keyword("K".to_owned()), span: span(2));
    let mixop_a = arena_mixop
        .intern(&MixopTree::Atom(atom_a.clone()))
        .unwrap();
    let mixop_b = arena_mixop
        .intern(&MixopTree::Atom(atom_b.clone()))
        .unwrap();
    assert_ne!(mixop_a, mixop_b);
    assert_eq!(arena_mixop.canon_id(mixop_a), arena_mixop.canon_id(mixop_b));
    let MixopTree::Atom(atom) = arena_mixop.to_tree(mixop_a) else { panic!("expected atom") };
    assert_eq!(atom, atom_a);
    let MixopTree::Atom(atom) = arena_mixop.to_tree(mixop_b) else { panic!("expected atom") };
    assert_eq!(atom, atom_b);
    let mixop = arena_mixop
        .intern(&MixopTree::Seq(vec![MixopTree::Arg, MixopTree::Arg]))
        .unwrap();
    let super::notation::MixopFlat::Seq(mixops) = arena_mixop.kind(mixop) else {
        panic!("expected sequence")
    };
    assert_eq!(mixops[0], mixops[1]);
    assert_eq!(arena_mixop.arity(mixop), 2);
    let mixop_empty = arena_mixop.intern(&MixopTree::Seq(vec![])).unwrap();
    assert_eq!(arena_mixop.arity(mixop_empty), 0);
    let mixop_tree = MixopTree::Brack(
        crate::phrase!(node: Atom::LParen, span: span(3)),
        Box::new(MixopTree::Infix(
            Box::new(MixopTree::Arg),
            crate::phrase!(node: Atom::Keyword("OP".to_owned()), span: span(4)),
            Box::new(MixopTree::Seq(vec![MixopTree::Arg, MixopTree::Arg])),
        )),
        crate::phrase!(node: Atom::RParen, span: span(5)),
    );
    let mixop = arena_mixop.intern(&mixop_tree).unwrap();
    assert_eq!(arena_mixop.arity(mixop), 3);
    assert_eq!(mixop_tree.arity(), 3);
    assert!(arena_mixop.matches_tree(mixop, &mixop_tree));
}

#[test]
fn notation_comparison_interleaves_arguments_across_arenas() {
    use super::notation::{Mixfix, MixopArena};
    use std::cmp::Ordering;
    let mixfix_l = Mixfix::seq(vec![
        Mixfix::arg(0),
        Mixfix::atom(crate::phrase!(node: Atom::Keyword("Z".to_owned()), span: span(1))),
    ]);
    let mixfix_r = Mixfix::seq(vec![
        Mixfix::arg(1),
        Mixfix::atom(crate::phrase!(node: Atom::Keyword("A".to_owned()), span: span(2))),
    ]);
    assert_eq!(mixfix_l.cmp_by(&mixfix_r, Ord::cmp), Ordering::Less);
    let mut arena_l = MixopArena::new();
    let mut arena_r = MixopArena::new();
    arena_r
        .intern(&super::notation::MixopTree::Seq(vec![]))
        .unwrap();
    let mixop_l = arena_l.intern(mixfix_l.mixop()).unwrap();
    let mixop_r = arena_r.intern(mixfix_r.mixop()).unwrap();
    let mixfix_flat_l = Mixfix::new_in(&arena_l, mixop_l, vec![0]).unwrap();
    let mixfix_flat_r = Mixfix::new_in(&arena_r, mixop_r, vec![1]).unwrap();
    assert_eq!(
        mixfix_flat_l.cmp_in_by(&arena_l, &mixfix_flat_r, &arena_r, Ord::cmp),
        Ordering::Less
    );
    assert!(Mixfix::<_, usize>::new_in(&arena_l, mixop_l, vec![]).is_err());
    assert!(Mixfix::new_in(&arena_l, mixop_l, vec![0, 1]).is_err());
}

#[test]
fn resetting_values_preserves_prepared_mixops() {
    use super::notation::MixopTree;
    let mut arena = Arena::new();
    let mixop_tree = MixopTree::Seq(vec![
        MixopTree::Atom(crate::phrase!(node: Atom::Keyword("CASE".to_owned()), span: span(1))),
        MixopTree::Arg,
    ]);
    let mixop_id = arena.arena_mixop_mut().intern(&mixop_tree).unwrap();
    let value = make::bool(&mut arena, true, span(2)).unwrap();
    assert!(get::bool(&arena, &value).unwrap());
    arena.reset_values();
    assert_eq!(arena.arena_mixop().arity(mixop_id), 1);
    assert_eq!(arena.arena_mixop().to_tree(mixop_id), mixop_tree);
    let value = make::bool(&mut arena, false, Span::default()).unwrap();
    assert!(!get::bool(&arena, &value).unwrap());
    assert_eq!(arena.span(&value), &Span::default());
    assert_eq!(value.span.index(), 0);
}

#[test]
fn mixop_encodings_preserve_contents_across_arenas() {
    use super::{
        encoding::Encoding,
        notation::{MixopArena, MixopFlat, MixopId, MixopTree, external},
    };
    let mut arena_a = MixopArena::new();
    let mixop_a = arena_a.intern(&MixopTree::Arg).unwrap();
    let json = external::encode(&arena_a, &mixop_a).unwrap();
    assert_eq!(json, serde_json::json!("Arg"));
    let mut arena_b = MixopArena::new();
    arena_b.intern(&MixopTree::Seq(vec![])).unwrap();
    let mixop_b: MixopId =
        external::decode_with(&mut arena_b, Encoding::ArenaIndependent, &json).unwrap();
    assert_ne!(mixop_a.index(), mixop_b.index());
    assert!(matches!(arena_b.kind(mixop_b), MixopFlat::Arg));
    let json_relative = external::encode_with(&arena_b, Encoding::ArenaRelative, &mixop_b).unwrap();
    assert_eq!(json_relative, serde_json::json!(mixop_b.index()));
    let mixop_again: MixopId =
        external::decode_with(&mut arena_b, Encoding::ArenaRelative, &json_relative).unwrap();
    assert_eq!(mixop_b, mixop_again);

    let mixop_tree = MixopTree::Brack(
        crate::phrase!(node: Atom::LParen, span: span(1)),
        Box::new(MixopTree::Infix(
            Box::new(MixopTree::Seq(vec![
                MixopTree::Arg,
                MixopTree::Atom(crate::phrase!(node: Atom::Keyword("K".to_owned()), span: span(2))),
            ])),
            crate::phrase!(node: Atom::Keyword("OP".to_owned()), span: span(3)),
            Box::new(MixopTree::Seq(vec![])),
        )),
        crate::phrase!(node: Atom::RParen, span: span(4)),
    );
    let mixop_a = arena_a.intern(&mixop_tree).unwrap();
    let json = external::encode(&arena_a, &mixop_a).unwrap();
    assert_eq!(json, serde_json::to_value(&mixop_tree).unwrap());
    let mixop_b: MixopId =
        external::decode_with(&mut arena_b, Encoding::ArenaIndependent, &json).unwrap();
    assert_eq!(serde_json::to_value(arena_b.to_tree(mixop_b)).unwrap(), json);
    // A flat body resolves its child handles using the same mode.
    let json_body = external::encode(&arena_a, arena_a.kind(mixop_a)).unwrap();
    assert_eq!(json_body, json);
    let body: MixopFlat =
        external::decode_with(&mut arena_b, Encoding::ArenaIndependent, &json_body).unwrap();
    assert_eq!(&body, arena_b.kind(mixop_b));
    let json_body_relative =
        external::encode_with(&arena_a, Encoding::ArenaRelative, arena_a.kind(mixop_a)).unwrap();
    let body: MixopFlat =
        external::decode_with(&mut arena_a, Encoding::ArenaRelative, &json_body_relative).unwrap();
    assert_eq!(&body, arena_a.kind(mixop_a));
    for encoding in [Encoding::ArenaIndependent, Encoding::ArenaRelative] {
        let mixops = vec![mixop_b, mixop_again, mixop_b];
        let json = external::encode_with(&arena_b, encoding, &mixops).unwrap();
        let mixops_again: Vec<MixopId> =
            external::decode_with(&mut arena_b, encoding, &json).unwrap();
        assert_eq!(mixops_again, mixops);
    }
}

#[test]
fn mixop_decoding_rejects_invalid_payloads() {
    use super::{
        encoding::Encoding,
        notation::{MixopArena, MixopId, external},
    };
    let mut arena_mixop = MixopArena::new();
    for json in [
        serde_json::json!(-1),
        serde_json::json!(4294967296u64),
        serde_json::json!("Arg"),
        serde_json::json!(0.5),
    ] {
        assert!(
            external::decode_with::<MixopId>(&mut arena_mixop, Encoding::ArenaRelative, &json)
                .is_err()
        );
    }
    for json in [
        serde_json::json!("Unknown"),
        serde_json::json!({"Brack": []}),
        serde_json::json!({"Seq": [0]}),
        serde_json::json!(0),
    ] {
        assert!(
            external::decode_with::<MixopId>(&mut arena_mixop, Encoding::ArenaIndependent, &json)
                .is_err()
        );
    }
}

#[test]
fn mixop_independent_encoding_handles_deep_trees() {
    use super::{
        encoding::Encoding,
        notation::{MixopArena, MixopId, MixopTree, external},
    };
    let mut mixop_tree = MixopTree::Arg;
    for _ in 0..256 {
        mixop_tree = MixopTree::Seq(vec![mixop_tree]);
    }
    let mut arena_a = MixopArena::new();
    let mixop_a = arena_a.intern(&mixop_tree).unwrap();
    let json = external::encode(&arena_a, &mixop_a).unwrap();
    let mut arena_b = MixopArena::new();
    let mixop_b: MixopId =
        external::decode_with(&mut arena_b, Encoding::ArenaIndependent, &json).unwrap();
    assert_eq!(arena_b.arity(mixop_b), 1);
    assert_eq!(external::encode(&arena_b, &mixop_b).unwrap(), json);
}

#[test]
fn shared_data_rendering_preserves_layout() {
    use crate::lang::traits::print::Print;
    let mut arena = Arena::new();
    let value_case = sample(&mut arena);
    let value = make::structure(
        &mut arena,
        Rc::new(typ::TypKind::Bool),
        vec![(
            crate::phrase!(node: Atom::Keyword("payload".to_owned()), span: span(12)),
            value_case,
        )],
        span(13),
    )
    .unwrap();
    assert_eq!(
        arena.to_string(&value),
        "{\n  payload `( INNER true argument Some(true) [\n      argument\n    ]  `)\n}"
    );
    let typ = typ::make::func(
        vec![],
        vec![typ::make::list(typ::make::bool()), typ::make::text()],
        typ::make::opt(typ::make::int()),
    );
    assert_eq!(Print::to_string(&typ), "(bool*, text) : int?");
}
