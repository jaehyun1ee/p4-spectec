use std::rc::Rc;

use num_bigint::BigInt;
use p4spec_rust::{
    domain::{
        atom::Atom,
        external_data::ExternalData,
        mixfix::Mixfix,
        source::{Region, Spanned},
    },
    lang::{
        il::ast::{self as il, TypKind},
        xl::num,
    },
    runtime::{
        r#type::typ::make as make_type,
        value::{ValueError, ValueKind, ValueTag, get, make},
    },
};

fn span(file: &str) -> Region {
    Region::for_file(file)
}

fn id(name: &str, file: &str) -> il::Id {
    Spanned::new(name.to_owned(), span(file))
}

fn atom(node: Atom, file: &str) -> il::Atom {
    Spanned::new(node, span(file))
}

#[test]
fn primitive_constructors_preserve_value_type_and_source_region() {
    let bool_value = make::bool(true, span("bool"));
    let nat_value = make::nat(BigInt::from(3), span("nat"));
    let int_value = make::int(BigInt::from(-4), span("int"));
    let num_value = make::num(num::T::Nat(BigInt::from(5)), span("num"));
    let text_value = make::text("hello".to_owned(), span("text"));

    assert_eq!(get::bool(&bool_value), Ok(true));
    assert_eq!(get::num(&nat_value), Ok(&num::T::Nat(BigInt::from(3))));
    assert_eq!(get::num(&int_value), Ok(&num::T::Int(BigInt::from(-4))));
    assert_eq!(get::num(&num_value), Ok(&num::T::Nat(BigInt::from(5))));
    assert_eq!(get::text(&text_value), Ok("hello"));
    assert_eq!(bool_value.ty, TypKind::BoolT);
    assert_eq!(nat_value.ty, TypKind::NumT(num::Typ::NatT));
    assert_eq!(int_value.ty, TypKind::NumT(num::Typ::IntT));
    assert_eq!(text_value.ty, TypKind::TextT);
    assert_eq!(bool_value.span, span("bool"));
    assert_eq!(text_value.span, span("text"));
}

#[test]
fn composite_constructors_keep_children_shared_by_rc() {
    let typ = make_type::var_type(id("Container", "type"), Vec::new());
    let child = make::bool(true, span("child"));
    let field = atom(Atom::Keyword("field".to_owned()), "field");
    let structure = make::structure(
        &typ,
        vec![(field.clone(), Rc::clone(&child))],
        span("struct"),
    );
    let case = make::case(&typ, Mixfix::Arg(Rc::clone(&child)), span("case"));
    let tuple = make::tuple(&typ, vec![Rc::clone(&child)], span("tuple"));
    let some = make::opt(&typ, Some(Rc::clone(&child)), span("some"));
    let none = make::opt(&typ, None, span("none"));
    let list = make::list(&typ, vec![Rc::clone(&child)], span("list"));
    let function = make::func(
        id("f", "function"),
        vec![id("T", "function")],
        vec![make_type::bool_type()],
        make_type::text_type(),
        span("function"),
    );
    let external_data = ExternalData::Variant("Token".to_owned(), None);
    let external = make::external(&typ, external_data.clone(), span("external"));

    let fields = get::structure(&structure).expect("get struct fields");
    assert_eq!(fields[0].0.node, field.node);
    assert!(Rc::ptr_eq(&fields[0].1, &child));
    assert!(Rc::ptr_eq(
        get::case(&case)
            .expect("get case")
            .args()
            .first()
            .expect("case argument"),
        &child
    ));
    assert!(Rc::ptr_eq(
        &get::tuple(&tuple).expect("get tuple")[0],
        &child
    ));
    assert!(Rc::ptr_eq(
        get::opt(&some).expect("get option").expect("some value"),
        &child
    ));
    assert!(get::opt(&none).expect("get option").is_none());
    assert!(Rc::ptr_eq(&get::list(&list).expect("get list")[0], &child));
    assert_eq!(get::func(&function).expect("get function").node, "f");
    assert_eq!(get::external(&external), Ok(&external_data));
    assert!(matches!(function.ty, TypKind::FuncT(_, _, _)));
    assert_eq!(structure.ty, typ.node);
    assert_eq!(structure.span, span("struct"));
}

#[test]
fn typed_getters_report_expected_and_actual_value_kinds() {
    let value = make::text("not bool".to_owned(), span("text"));

    assert_eq!(
        get::bool(&value),
        Err(ValueError::UnexpectedKind {
            expected: ValueTag::Bool,
            actual: ValueTag::Text,
        })
    );
}

#[test]
fn extractors_validate_indices_and_exact_value_counts() {
    let value_a = make::bool(true, span("a"));
    let value_b = make::bool(false, span("b"));
    let value_c = make::text("c".to_owned(), span("c"));
    let values = vec![
        Rc::clone(&value_a),
        Rc::clone(&value_b),
        Rc::clone(&value_c),
    ];

    assert!(Rc::ptr_eq(
        get::nth(&values, 1).expect("second value"),
        &value_b
    ));
    assert!(matches!(
        get::nth(&values, 3),
        Err(ValueError::IndexOutOfBounds { index: 3, len: 3 })
    ));
    assert!(Rc::ptr_eq(
        get::one(std::slice::from_ref(&value_a)).expect("one value"),
        &value_a
    ));
    let (first, second) = get::two(&values[..2]).expect("two values");
    assert!(Rc::ptr_eq(first, &value_a));
    assert!(Rc::ptr_eq(second, &value_b));
    let (first, second, third) = get::three(&values).expect("three values");
    assert!(Rc::ptr_eq(first, &value_a));
    assert!(Rc::ptr_eq(second, &value_b));
    assert!(Rc::ptr_eq(third, &value_c));
    assert!(matches!(
        get::one(&values),
        Err(ValueError::ExpectedCount {
            expected: 1,
            actual: 3,
        })
    ));
}

#[test]
fn every_runtime_value_constructor_uses_the_runtime_value_kind() {
    let typ = make_type::bool_type();
    let child = make::bool(false, Region::none());
    let values = [
        make::bool(false, Region::none()),
        make::nat(BigInt::from(0), Region::none()),
        make::text(String::new(), Region::none()),
        make::structure(&typ, Vec::new(), Region::none()),
        make::case(&typ, Mixfix::Seq(Vec::new()), Region::none()),
        make::tuple(&typ, Vec::new(), Region::none()),
        make::opt(&typ, None, Region::none()),
        make::list(&typ, vec![child], Region::none()),
        make::func(
            id("f", "function"),
            Vec::new(),
            Vec::new(),
            make_type::bool_type(),
            Region::none(),
        ),
        make::external(&typ, ExternalData::Null, Region::none()),
    ];

    assert!(matches!(values[0].kind, ValueKind::BoolV(_)));
    assert!(matches!(values[1].kind, ValueKind::NumV(_)));
    assert!(matches!(values[2].kind, ValueKind::TextV(_)));
    assert!(matches!(values[3].kind, ValueKind::StructV(_)));
    assert!(matches!(values[4].kind, ValueKind::CaseV(_)));
    assert!(matches!(values[5].kind, ValueKind::TupleV(_)));
    assert!(matches!(values[6].kind, ValueKind::OptV(_)));
    assert!(matches!(values[7].kind, ValueKind::ListV(_)));
    assert!(matches!(values[8].kind, ValueKind::FuncV(_)));
    assert!(matches!(values[9].kind, ValueKind::ExternV(_)));
}
