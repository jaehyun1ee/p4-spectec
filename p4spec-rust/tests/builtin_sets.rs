use num_bigint::BigInt;
use p4spec_rust::{
    domain::{
        atom::Atom,
        mixfix::{Mixfix, Mixop},
        source::{Region, Spanned},
    },
    interface::builtin::sets,
    runtime::{
        r#type::typ::make as make_type,
        value::{ValueRef, get, make},
    },
};

fn span(file: &str) -> Region {
    Region::for_file(file)
}

fn integer(value: i64, file: &str) -> ValueRef {
    make::int(BigInt::from(value), span(file))
}

fn set_mixop() -> Mixop {
    Mixfix::Brack(
        Spanned::new(Atom::LBrace, Region::none()),
        Box::new(Mixfix::Arg(())),
        Spanned::new(Atom::RBrace, Region::none()),
    )
}

fn set(values: Vec<ValueRef>) -> ValueRef {
    let element_type = make_type::int_type();
    let list = make::list(
        &make_type::list_type(element_type.clone()),
        values,
        span("set-elements"),
    );
    let value_case = Mixop::fill(&set_mixop(), [list]).expect("one set argument");
    make::case(
        &make_type::var_type(
            Spanned::new("set".to_owned(), Region::none()),
            vec![element_type],
        ),
        value_case,
        span("set"),
    )
}

fn set_type() -> p4spec_rust::lang::il::ast::Typ {
    make_type::var_type(
        Spanned::new("set".to_owned(), Region::none()),
        vec![make_type::int_type()],
    )
}

fn integers(value: &ValueRef) -> Vec<BigInt> {
    let value_case = get::case(value).expect("set case");
    let args = value_case.args();
    let elements = get::list(args[0]).expect("set elements");
    elements
        .iter()
        .map(|value| match get::num(value).expect("integer") {
            p4spec_rust::lang::xl::num::T::Int(value) => value.clone(),
            p4spec_rust::lang::xl::num::T::Nat(_) => panic!("expected integer"),
        })
        .collect()
}

#[test]
fn intersection_union_and_difference_use_semantic_ordered_values() {
    let callsite = span("set-call");
    let type_args = vec![make_type::int_type()];
    let left = set(vec![integer(2, "left-two"), integer(1, "left-one")]);
    let right = set(vec![integer(2, "right-two"), integer(3, "right-three")]);
    let mut recorded = Vec::new();

    let intersection = {
        let mut add = |value| recorded.push(value);
        sets::intersect_set(
            &mut add,
            &callsite,
            &type_args,
            &[left.clone(), right.clone()],
        )
        .unwrap()
    };
    assert_eq!(integers(&intersection), vec![2.into()]);
    assert_eq!(recorded.len(), 2);

    let mut add = |_| {};
    let union = sets::union_set(
        &mut add,
        &callsite,
        &type_args,
        &[left.clone(), right.clone()],
    )
    .unwrap();
    assert_eq!(integers(&union), vec![1.into(), 2.into(), 3.into()]);

    let difference = sets::diff_set(&mut add, &callsite, &type_args, &[left, right]).unwrap();
    assert_eq!(integers(&difference), vec![1.into()]);
}

#[test]
fn unions_subset_and_equality_follow_ocaml_set_semantics() {
    let callsite = span("set-call");
    let type_args = vec![make_type::int_type()];
    let singleton = set(vec![integer(1, "singleton")]);
    let superset = set(vec![integer(2, "two"), integer(1, "one")]);
    let sets_value = make::list(
        &make_type::list_type(set_type()),
        vec![singleton.clone(), superset.clone()],
        span("sets"),
    );
    let mut add = |_| {};

    let union = sets::unions_set(&mut add, &callsite, &type_args, &[sets_value]).unwrap();
    assert_eq!(integers(&union), vec![1.into(), 2.into()]);

    let subset = sets::sub_set(
        &mut add,
        &callsite,
        &type_args,
        &[singleton.clone(), superset.clone()],
    )
    .unwrap();
    assert_eq!(get::bool(&subset), Ok(true));

    let equal = sets::eq_set(
        &mut add,
        &callsite,
        &type_args,
        &[singleton, set(vec![integer(1, "different-span")])],
    )
    .unwrap();
    assert_eq!(get::bool(&equal), Ok(true));
}

#[test]
fn malformed_set_is_a_typed_builtin_error() {
    let error = sets::union_set(
        &mut |_| {},
        &span("bad-call"),
        &[make_type::int_type()],
        &[integer(1, "not-a-set"), set(vec![integer(1, "element")])],
    )
    .unwrap_err();
    assert_eq!(error.span, span("bad-call"));
    assert!(error.message.contains("expected a set"));
}
