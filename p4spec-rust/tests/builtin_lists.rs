use num_bigint::BigInt;
use p4spec_rust::{
    domain::source::Region,
    interface::builtin::{BuiltinError, lists},
    lang::{il::ast::TypKind, xl::num},
    runtime::{
        r#type::typ::make as make_type,
        value::{ValueRef, get, make},
    },
};

fn span(file: &str) -> Region {
    Region::for_file(file)
}

fn integer(value: i64) -> ValueRef {
    make::int(BigInt::from(value), span("integer"))
}

fn list(values: Vec<ValueRef>) -> ValueRef {
    make::list(
        &make_type::list_type(make_type::int_type()),
        values,
        span("list"),
    )
}

fn integer_value(value: &ValueRef) -> BigInt {
    match get::num(value).expect("number") {
        num::T::Nat(value) | num::T::Int(value) => value.clone(),
    }
}

fn integers(value: &ValueRef) -> Vec<BigInt> {
    get::list(value)
        .expect("list")
        .iter()
        .map(integer_value)
        .collect()
}

#[test]
fn reverse_concat_and_distinct_use_semantic_values() {
    let callsite = span("list-call");
    let type_args = vec![make_type::int_type()];
    let mut add = |_| {};
    let original = list(vec![integer(1), integer(2), integer(3)]);

    let reversed = lists::rev_(
        &mut add,
        &callsite,
        &type_args,
        std::slice::from_ref(&original),
    )
    .unwrap();
    assert_eq!(integers(&reversed), vec![3.into(), 2.into(), 1.into()]);

    let matrix = make::list(
        &make_type::list_type(make_type::list_type(make_type::int_type())),
        vec![list(vec![integer(1), integer(2)]), list(vec![integer(3)])],
        span("matrix"),
    );
    let concatenated = lists::concat_(&mut add, &callsite, &type_args, &[matrix]).unwrap();
    assert_eq!(integers(&concatenated), vec![1.into(), 2.into(), 3.into()]);

    let duplicate = lists::distinct_(
        &mut add,
        &callsite,
        &type_args,
        &[list(vec![integer(1), integer(2), integer(1)])],
    )
    .unwrap();
    assert_eq!(get::bool(&duplicate), Ok(false));
    let distinct = lists::distinct_(
        &mut add,
        &callsite,
        &type_args,
        &[list(vec![integer(1), integer(2)])],
    )
    .unwrap();
    assert_eq!(get::bool(&distinct), Ok(true));
}

#[test]
fn partition_and_assoc_preserve_first_match_and_record_nested_results() {
    let callsite = span("partition-call");
    let type_args = vec![make_type::int_type()];
    let mut recorded = Vec::new();
    let partitioned = {
        let mut add = |value| recorded.push(value);
        lists::partition_(
            &mut add,
            &callsite,
            &type_args,
            &[list(vec![integer(1), integer(2), integer(3)]), integer(2)],
        )
        .unwrap()
    };
    let parts = get::tuple(&partitioned).expect("partition tuple");
    assert_eq!(integers(&parts[0]), vec![1.into(), 2.into()]);
    assert_eq!(integers(&parts[1]), vec![3.into()]);
    assert_eq!(recorded.len(), 3);

    let pair_type = make_type::tuple_type(vec![make_type::int_type(), make_type::text_type()]);
    let pair = |key: i64, value: &str| {
        make::tuple(
            &pair_type,
            vec![
                integer(key),
                make::text(value.to_owned(), span("pair-value")),
            ],
            span("pair"),
        )
    };
    let pairs = make::list(
        &make_type::list_type(pair_type.clone()),
        vec![pair(1, "first"), pair(1, "second")],
        span("pairs"),
    );
    let mut add = |_| {};
    let found = lists::assoc_(
        &mut add,
        &callsite,
        &[make_type::int_type(), make_type::text_type()],
        &[integer(1), pairs],
    )
    .unwrap();
    assert_eq!(
        get::text(get::opt(&found).unwrap().expect("found value")).unwrap(),
        "first",
    );
}

#[test]
fn sort_and_transpose_follow_numeric_key_and_rectangular_matrix_rules() {
    let callsite = span("matrix-call");
    let pair_type = make_type::tuple_type(vec![make_type::nat_type(), make_type::text_type()]);
    let pair = |key: i64, value: &str| {
        make::tuple(
            &pair_type,
            vec![
                make::nat(BigInt::from(key), span("key")),
                make::text(value.to_owned(), span("value")),
            ],
            span("pair"),
        )
    };
    let pairs = make::list(
        &make_type::list_type(pair_type.clone()),
        vec![pair(2, "b"), pair(1, "a")],
        span("pairs"),
    );
    let mut add = |_| {};
    let sorted = lists::sort_(&mut add, &callsite, &[make_type::text_type()], &[pairs]).unwrap();
    let sorted = get::list(&sorted).unwrap();
    assert_eq!(
        integer_value(&get::tuple(&sorted[0]).unwrap()[0]),
        BigInt::from(1)
    );
    assert_eq!(
        integer_value(&get::tuple(&sorted[1]).unwrap()[0]),
        BigInt::from(2)
    );

    let matrix = make::list(
        &make_type::list_type(make_type::list_type(make_type::int_type())),
        vec![
            list(vec![integer(1), integer(2)]),
            list(vec![integer(3), integer(4)]),
        ],
        span("matrix"),
    );
    let transposed =
        lists::transpose_(&mut add, &callsite, &[make_type::int_type()], &[matrix]).unwrap();
    let rows = get::list(&transposed).unwrap();
    assert_eq!(integers(&rows[0]), vec![1.into(), 3.into()]);
    assert_eq!(integers(&rows[1]), vec![2.into(), 4.into()]);

    let ragged = make::list(
        &make_type::list_type(make_type::list_type(make_type::int_type())),
        vec![list(vec![integer(1)]), list(vec![integer(2), integer(3)])],
        span("ragged"),
    );
    assert_eq!(
        lists::transpose_(&mut add, &callsite, &[make_type::int_type()], &[ragged],),
        Err(BuiltinError::new(
            Region::none(),
            "cannot transpose a matrix of values",
        )),
    );
}

#[test]
fn list_builtin_results_keep_the_ocaml_type_shapes() {
    let mut add = |_| {};
    let partitioned = lists::partition_(
        &mut add,
        &span("call"),
        &[make_type::text_type()],
        &[
            make::list(
                &make_type::list_type(make_type::text_type()),
                Vec::new(),
                span("empty"),
            ),
            integer(0),
        ],
    )
    .unwrap();
    assert!(matches!(partitioned.ty, TypKind::TupleT(ref types) if types.len() == 2));
}
