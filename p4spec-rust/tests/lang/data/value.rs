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
    assert_ne!(last.node, duplicate.node);
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
