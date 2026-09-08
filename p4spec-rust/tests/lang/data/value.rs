//! Shared runtime value annotations, ordering, and lossless wire transport

use num_bigint::BigInt;
use p4spec_rust::{
    lang::{
        common::{
            notation::{atom::Atom, mixfix::Mixfix},
            source::{Position, Span},
        },
        data::{
            typ,
            value::{ValueArena, ValueError, ValueKind, ValueTag, get, make},
        },
        il::print::string_of_value,
        xl::num::{Natural, Number},
    },
    wire::ocaml::lang::il::{ValueCodec, ValueEnvelopeCodec},
    yojson::ExternalData,
};
use std::{
    cmp::Ordering,
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

fn span(file: &str, line: i64) -> Span {
    Span::new(Position::new(file, line, 0), Position::new(file, line, 1))
}

#[test]
fn test_arena_growth_preserves_handles_and_structural_order() {
    let mut arena = ValueArena::new();
    let last = make::text(&mut arena, "z".into(), span("z.p4", 9)).unwrap();
    let first = make::text(&mut arena, "a".into(), span("a.p4", 1)).unwrap();
    for _ in 0..4096 {
        make::bool(&mut arena, true, Span::default()).unwrap();
    }
    let duplicate = make::text(&mut arena, "z".into(), span("z.p4", 9)).unwrap();
    assert_eq!(get::text(&arena, &last), Ok("z"));
    assert_eq!(arena.compare(&first, &last), Ordering::Less);
    assert!(arena.equal(&last, &duplicate));
    assert_eq!(last, duplicate);
}

#[test]
fn test_interning_shares_root_body_and_exact_annotations() {
    let mut arena = ValueArena::new();
    let value = make::bool(&mut arena, true, span("a", 1)).unwrap();
    let duplicate = make::bool(&mut arena, true, span("a", 1)).unwrap();
    let moved = make::bool(&mut arena, true, span("b", 2)).unwrap();
    let annotated = make::new(
        &mut arena,
        ValueKind::Bool(true),
        typ::make::text().node,
        span("a", 1),
    )
    .unwrap();
    assert_eq!(value, duplicate);
    assert_eq!(value.node, moved.node);
    assert_eq!(value.node, annotated.node);
    assert_eq!(value.note, moved.note);
    assert_eq!(value.span, annotated.span);
    assert_ne!(value.span, moved.span);
    assert_ne!(value.note, annotated.note);
    assert!(!arena.equal(&value, &moved));
    assert!(!arena.equal(&value, &annotated));
}

#[test]
fn test_semantic_identity_shares_nested_annotations_but_exact_parents_do_not() {
    let mut arena = ValueArena::new();
    let child = make::bool(&mut arena, true, span("child", 1)).unwrap();
    let moved = arena.relocate(child, span("child", 2)).unwrap();
    let annotated = arena.annotate(child, typ::make::text().node).unwrap();
    let mut parents = vec![];
    for child in [child, moved, annotated] {
        let parent =
            make::list(&mut arena, &typ::make::bool(), vec![child], Span::default()).unwrap();
        let duplicate =
            make::list(&mut arena, &typ::make::bool(), vec![child], Span::default()).unwrap();
        assert_eq!(parent, duplicate);
        assert_eq!(get::list(&arena, &parent).unwrap(), &[child]);
        parents.push(parent);
    }
    for parent in &parents[1..] {
        assert_ne!(parents[0].node, parent.node);
        assert_eq!(arena.semantic_id(&parents[0]), arena.semantic_id(parent));
        assert!(!arena.equal(&parents[0], parent));
    }
    let grandparents = parents
        .iter()
        .map(|parent| {
            make::opt(
                &mut arena,
                &typ::make::bool(),
                Some(*parent),
                Span::default(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    assert_ne!(grandparents[0].node, grandparents[1].node);
    assert_eq!(
        arena.semantic_id(&grandparents[0]),
        arena.semantic_id(&grandparents[1])
    );
}

#[test]
fn test_interning_preserves_mixfix_and_function_label_locations() {
    let mut arena = ValueArena::new();
    let child = make::bool(&mut arena, true, Span::default()).unwrap();
    let case = |line| {
        let label =
            |name| p4spec_rust::phrase!(node: Atom::keyword(name), span: span("label", line));
        Mixfix::Infix(
            Box::new(Mixfix::Brack(
                label("("),
                Box::new(Mixfix::Arg(child)),
                label(")"),
            )),
            label("+"),
            Box::new(Mixfix::Seq(vec![Mixfix::Atom(label("END"))])),
        )
    };
    let left = make::case_(&mut arena, &typ::make::bool(), case(1), Span::default()).unwrap();
    let same = make::case_(&mut arena, &typ::make::bool(), case(1), Span::default()).unwrap();
    let right = make::case_(&mut arena, &typ::make::bool(), case(2), Span::default()).unwrap();
    assert_eq!(left, same);
    assert_ne!(left.node, right.node);
    assert_eq!(arena.semantic_id(&left), arena.semantic_id(&right));
    assert_ne!(
        ValueCodec::encode(&arena, &left).unwrap(),
        ValueCodec::encode(&arena, &right).unwrap()
    );
    let mut functions = vec![];
    for line in [1, 1, 2] {
        let id = p4spec_rust::phrase!(node: "f".to_owned(), span: span("function", line));
        functions.push(
            make::func(
                &mut arena,
                id,
                vec![],
                vec![],
                typ::make::bool(),
                Span::default(),
            )
            .unwrap(),
        );
    }
    assert_eq!(functions[0], functions[1]);
    assert_ne!(functions[0].node, functions[2].node);
    assert_eq!(
        arena.semantic_id(&functions[0]),
        arena.semantic_id(&functions[2])
    );
}

#[test]
fn test_semantic_identity_preserves_variants_names_and_order() {
    let mut arena = ValueArena::new();
    let typ = typ::make::bool();
    let nat = make::nat(&mut arena, 1_u64.into(), Span::default()).unwrap();
    let int = make::int(&mut arena, 1.into(), Span::default()).unwrap();
    let label = |atom| p4spec_rust::phrase!(node: atom, span: Span::default());
    let mut values = vec![nat, int];
    for children in [vec![nat, int], vec![int, nat], vec![nat, nat], vec![nat]] {
        values.push(make::list(&mut arena, &typ, children.clone(), Span::default()).unwrap());
        values.push(make::tuple(&mut arena, &typ, children, Span::default()).unwrap());
    }
    for atom in [
        Atom::Keyword("X".into()),
        Atom::Tag("X".into()),
        Atom::Operator("X".into()),
        Atom::Arrow,
    ] {
        values.push(
            make::case_(&mut arena, &typ, Mixfix::Atom(label(atom)), Span::default()).unwrap(),
        );
    }
    for fields in [
        vec![
            (label(Atom::keyword("a")), nat),
            (label(Atom::keyword("b")), int),
        ],
        vec![
            (label(Atom::keyword("b")), int),
            (label(Atom::keyword("a")), nat),
        ],
        vec![
            (label(Atom::keyword("a")), nat),
            (label(Atom::keyword("a")), int),
        ],
    ] {
        values.push(make::structure(&mut arena, &typ, fields, Span::default()).unwrap());
    }
    for name in ["f", "g"] {
        values.push(
            make::func(
                &mut arena,
                p4spec_rust::phrase!(node: name.to_owned(), span: Span::default()),
                vec![],
                vec![],
                typ.clone(),
                Span::default(),
            )
            .unwrap(),
        );
    }
    values.push(make::opt(&mut arena, &typ, None, Span::default()).unwrap());
    values.push(make::opt(&mut arena, &typ, Some(nat), Span::default()).unwrap());
    for (index, left) in values.iter().enumerate() {
        for right in &values[index + 1..] {
            assert_ne!(left.node, right.node);
            assert_ne!(arena.semantic_id(left), arena.semantic_id(right));
        }
    }
}

#[test]
fn test_external_interning_uses_normalized_float_equality_and_ordered_associations() {
    let mut arena = ValueArena::new();
    let typ = typ::make::text();
    for (left, right) in [
        (ExternalData::Float(-0.0), ExternalData::Float(0.0)),
        (
            ExternalData::Float(f64::NAN),
            ExternalData::Float(f64::from_bits(0x7ff8_0000_0000_0001)),
        ),
    ] {
        let left = make::external(&mut arena, &typ, left, Span::default()).unwrap();
        let right = make::external(&mut arena, &typ, right, Span::default()).unwrap();
        assert_eq!(left, right);
        assert_eq!(arena.semantic_id(&left), arena.semantic_id(&right));
    }
    let mut values = vec![];
    for data in [
        ExternalData::Assoc(vec![
            ("x".into(), ExternalData::Int(1)),
            ("x".into(), ExternalData::Int(2)),
        ]),
        ExternalData::Assoc(vec![
            ("x".into(), ExternalData::Int(2)),
            ("x".into(), ExternalData::Int(1)),
        ]),
        ExternalData::Assoc(vec![("x".into(), ExternalData::Int(1))]),
    ] {
        values.push(make::external(&mut arena, &typ, data, Span::default()).unwrap());
    }
    for (index, left) in values.iter().enumerate() {
        for right in &values[index + 1..] {
            assert_ne!(arena.semantic_id(left), arena.semantic_id(right));
        }
    }
}

#[test]
fn test_value_equality_includes_type_and_source() {
    let mut arena = ValueArena::new();
    let value = make::bool(&mut arena, true, span("a.p4", 1)).unwrap();
    let different_type = make::new(
        &mut arena,
        ValueKind::Bool(true),
        typ::make::text().node,
        span("a.p4", 1),
    )
    .unwrap();
    let different_source = make::bool(&mut arena, true, span("b.p4", 9)).unwrap();
    assert!(!arena.equal(&value, &different_type));
    assert!(!arena.equal(&value, &different_source));
    assert!(arena.syntax_eq(&value, &different_type));
    assert!(arena.syntax_eq(&value, &different_source));
}

#[test]
fn test_type_interning_preserves_nested_annotation_locations() {
    let mut arena = ValueArena::new();
    let type_at = |line| {
        let id = p4spec_rust::phrase!(node: "T".to_owned(), span: span("type", line));
        let nested = p4spec_rust::phrase!(node: typ::TypKind::Bool, span: span("argument", line));
        typ::make::var(id, vec![nested]).node
    };
    let first = make::new(
        &mut arena,
        ValueKind::Bool(true),
        type_at(1),
        Span::default(),
    )
    .unwrap();
    let duplicate = make::new(
        &mut arena,
        ValueKind::Bool(true),
        type_at(1),
        Span::default(),
    )
    .unwrap();
    let other = make::new(
        &mut arena,
        ValueKind::Bool(true),
        type_at(2),
        Span::default(),
    )
    .unwrap();
    assert_eq!(first, duplicate);
    assert_eq!(first.node, other.node);
    assert_ne!(first.note, other.note);
    assert_eq!(arena.typ(&other), &type_at(2));
    assert_eq!(arena.semantic_id(&first), arena.semantic_id(&other));
    assert!(!arena.equal(&first, &other));
}

#[test]
fn test_value_order_uses_variant_and_payload_order() {
    let mut arena = ValueArena::new();
    let text = make::text(&mut arena, String::new(), Span::default()).unwrap();
    let integer = make::int(&mut arena, BigInt::from(0), Span::default()).unwrap();
    let natural = make::nat(&mut arena, Natural::from(0_u64), Span::default()).unwrap();
    let boolean = make::bool(&mut arena, false, Span::default()).unwrap();
    for (left, right) in [(boolean, natural), (natural, integer), (integer, text)] {
        assert_eq!(arena.compare(&left, &right), Ordering::Less);
    }
}

#[test]
fn test_function_values_order_by_name_and_preserve_identifier_spans() {
    let mut arena = ValueArena::new();
    let id_b = p4spec_rust::phrase!(node: "b".to_owned(), span: span("b.spec", 2));
    let id_a = p4spec_rust::phrase!(node: "a".to_owned(), span: span("a.spec", 1));
    let func_b = make::func(
        &mut arena,
        id_b,
        vec![],
        vec![],
        typ::make::bool(),
        Span::default(),
    )
    .unwrap();
    let func_a = make::func(
        &mut arena,
        id_a,
        vec![],
        vec![],
        typ::make::bool(),
        Span::default(),
    )
    .unwrap();
    assert_eq!(arena.compare(&func_a, &func_b), Ordering::Less);
    assert_eq!(
        arena.location(get::func(&arena, &func_a).unwrap().span),
        &span("a.spec", 1)
    );
}

#[test]
fn test_external_float_order_normalizes_signed_zero() {
    let mut arena = ValueArena::new();
    let typ = typ::make::text();
    let negative =
        make::external(&mut arena, &typ, ExternalData::Float(-0.0), Span::default()).unwrap();
    let positive =
        make::external(&mut arena, &typ, ExternalData::Float(0.0), Span::default()).unwrap();
    assert!(arena.equal(&negative, &positive));
    let hash = |value: ExternalData| {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    };
    assert_eq!(
        hash(ExternalData::Float(-0.0)),
        hash(ExternalData::Float(0.0))
    );
}

#[test]
fn test_constructors_preserve_runtime_type_and_span() {
    let mut arena = ValueArena::new();
    let value = make::num(
        &mut arena,
        Number::Nat(Natural::from(7_u64)),
        span("program.p4", 4),
    )
    .unwrap();
    assert_eq!(arena.span(&value), &span("program.p4", 4));
    assert_eq!(arena.typ(&value), &typ::make::nat().node);
    assert_eq!(
        get::num(&arena, &value),
        Ok(&Number::Nat(Natural::from(7_u64)))
    );
}

#[test]
fn test_getters_report_expected_and_actual_kinds() {
    let mut arena = ValueArena::new();
    let value = make::text(&mut arena, "payload".to_owned(), Span::default()).unwrap();
    assert_eq!(
        get::bool(&arena, &value),
        Err(ValueError::UnexpectedKind {
            expected: ValueTag::Bool,
            actual: ValueTag::Text
        })
    );
    assert_eq!(
        get::one(&[]),
        Err(ValueError::ExpectedCount {
            expected: 1,
            actual: 0
        })
    );
}

#[test]
fn test_relocation_preserves_body_and_nested_annotations() {
    let mut arena = ValueArena::new();
    let child = make::bool(&mut arena, true, span("child.p4", 3)).unwrap();
    let value = make::list(
        &mut arena,
        &typ::make::list(typ::make::bool()),
        vec![child],
        span("parent.p4", 1),
    )
    .unwrap();
    let moved = arena.relocate(value, span("moved.p4", 9)).unwrap();
    assert_eq!(value.node, moved.node);
    assert_eq!(value.note, moved.note);
    assert_eq!(arena.span(&value), &span("parent.p4", 1));
    assert_eq!(get::list(&arena, &moved).unwrap(), &[child]);
    assert!(!arena.equal(&value, &moved));
    assert!(arena.syntax_eq(&value, &moved));
    assert_eq!(string_of_value(&arena, &child), "true");
}

#[test]
fn test_full_comparison_ignores_label_spans_but_includes_child_annotations() {
    let mut arena = ValueArena::new();
    let child = make::bool(&mut arena, true, span("child.p4", 3)).unwrap();
    let other = arena.annotate(child, typ::make::text().node).unwrap();
    let label = |file| p4spec_rust::phrase!(node: Atom::keyword("field"), span: span(file, 7));
    let typ = typ::make::bool();
    let left =
        make::structure(&mut arena, &typ, vec![(label("a"), child)], Span::default()).unwrap();
    let right =
        make::structure(&mut arena, &typ, vec![(label("b"), child)], Span::default()).unwrap();
    let changed =
        make::structure(&mut arena, &typ, vec![(label("b"), other)], Span::default()).unwrap();
    assert!(arena.equal(&left, &right));
    assert!(!arena.equal(&left, &changed));
    assert!(arena.syntax_eq(&left, &changed));
    assert_ne!(left.node, right.node);
    assert_ne!(right.node, changed.node);
    assert_ne!(
        ValueCodec::encode(&arena, &left).unwrap(),
        ValueCodec::encode(&arena, &right).unwrap()
    );
}

#[test]
fn test_all_value_variants_preserve_nested_wire_annotations() {
    let mut arena = ValueArena::new();
    let typ = typ::make::bool();
    let boolean = make::bool(&mut arena, true, span("bool", 1)).unwrap();
    let natural = make::nat(&mut arena, 7_u64.into(), span("nat", 2)).unwrap();
    let integer = make::int(&mut arena, (-3).into(), span("int", 3)).unwrap();
    let text = make::text(&mut arena, "nested".into(), span("text", 4)).unwrap();
    let label =
        |name, line| p4spec_rust::phrase!(node: Atom::keyword(name), span: span("labels", line));
    let structure = make::structure(
        &mut arena,
        &typ,
        vec![(label("field", 1), text)],
        span("struct", 5),
    )
    .unwrap();
    let notation = Mixfix::Infix(
        Box::new(Mixfix::Brack(
            label("(", 2),
            Box::new(Mixfix::Arg(natural)),
            label(")", 3),
        )),
        label("+", 4),
        Box::new(Mixfix::Seq(vec![
            Mixfix::Arg(integer),
            Mixfix::Atom(label("END", 5)),
        ])),
    );
    let case = make::case_(&mut arena, &typ, notation, span("case", 6)).unwrap();
    let tuple = make::tuple(&mut arena, &typ, vec![boolean, structure], span("tuple", 7)).unwrap();
    let none = make::opt(&mut arena, &typ, None, span("none", 8)).unwrap();
    let some = make::opt(&mut arena, &typ, Some(case), span("some", 9)).unwrap();
    let list = make::list(&mut arena, &typ, vec![tuple, some], span("list", 10)).unwrap();
    let id = p4spec_rust::phrase!(node: "function".to_owned(), span: span("identifier", 11));
    let func = make::func(
        &mut arena,
        id,
        vec![],
        vec![],
        typ.clone(),
        span("func", 12),
    )
    .unwrap();
    let external =
        make::external(&mut arena, &typ, ExternalData::Null, span("extern", 13)).unwrap();
    for value in [
        boolean, natural, integer, text, structure, case, tuple, none, some, list, func, external,
    ] {
        let json = ValueCodec::encode(&arena, &value).unwrap();
        let decoded = ValueCodec::decode(&mut arena, &json).unwrap();
        assert!(arena.equal(&value, &decoded));
        assert_eq!(ValueCodec::encode(&arena, &decoded).unwrap(), json);
        let bytes = ValueEnvelopeCodec::encode(&arena, &value).unwrap();
        let decoded = ValueEnvelopeCodec::decode(&mut arena, &bytes).unwrap();
        assert!(arena.equal(&value, &decoded));
        assert_eq!(ValueEnvelopeCodec::encode(&arena, &decoded).unwrap(), bytes);
    }
}
