use num_bigint::BigInt;
use p4spec_rust::{
    domain::{
        atom::Atom,
        mixfix::{Mixfix, Mixop},
        source::{Region, Spanned},
    },
    interface::builtin::maps,
    lang::xl::num,
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

fn text(value: &str) -> ValueRef {
    make::text(value.to_owned(), span("text"))
}

fn pair_mixop() -> Mixop {
    Mixfix::Seq(vec![
        Mixfix::Arg(()),
        Mixfix::Atom(Spanned::new(Atom::Operator(":".to_owned()), Region::none())),
        Mixfix::Arg(()),
    ])
}

fn map_mixop() -> Mixop {
    Mixfix::Brack(
        Spanned::new(Atom::LBrace, Region::none()),
        Box::new(Mixfix::Arg(())),
        Spanned::new(Atom::RBrace, Region::none()),
    )
}

fn pair(key: ValueRef, value: ValueRef) -> ValueRef {
    let pair_type = make_type::var_type(
        Spanned::new("pair".to_owned(), Region::none()),
        vec![make_type::int_type(), make_type::text_type()],
    );
    make::case(
        &pair_type,
        Mixop::fill(&pair_mixop(), [key, value]).expect("two pair arguments"),
        span("pair"),
    )
}

fn map(pairs: Vec<ValueRef>) -> ValueRef {
    let pair_type = make_type::var_type(
        Spanned::new("pair".to_owned(), Region::none()),
        vec![make_type::int_type(), make_type::text_type()],
    );
    let pairs = make::list(&make_type::list_type(pair_type), pairs, span("map-pairs"));
    let map_type = make_type::var_type(
        Spanned::new("map".to_owned(), Region::none()),
        vec![make_type::int_type(), make_type::text_type()],
    );
    make::case(
        &map_type,
        Mixop::fill(&map_mixop(), [pairs]).expect("one map argument"),
        span("map"),
    )
}

fn canonical_pair(key: ValueRef, value: ValueRef) -> ValueRef {
    let pair_type = make_type::var_type(
        Spanned::new("pair".to_owned(), Region::none()),
        vec![make_type::int_type(), make_type::text_type()],
    );
    let pair_mixop = Mixfix::Seq(vec![
        Mixfix::Arg(()),
        Mixfix::Atom(Spanned::new(Atom::Operator(":".to_owned()), Region::none())),
        Mixfix::Arg(()),
    ]);
    make::case(
        &pair_type,
        Mixop::fill(&pair_mixop, [key, value]).expect("two pair arguments"),
        span("pair"),
    )
}

fn entries(value: &ValueRef) -> Vec<(BigInt, String)> {
    let map_case = get::case(value).expect("map case");
    let map_args = map_case.args();
    let pairs = get::list(map_args[0]).expect("map pairs");
    pairs
        .iter()
        .map(|pair| {
            let pair_case = get::case(pair).expect("pair case");
            let args = pair_case.args();
            let key = match get::num(args[0]).expect("integer key") {
                num::T::Int(key) => key.clone(),
                num::T::Nat(_) => panic!("expected integer key"),
            };
            let value = get::text(args[1]).expect("text value").to_owned();
            (key, value)
        })
        .collect()
}

fn type_args() -> Vec<p4spec_rust::lang::il::ast::Typ> {
    vec![make_type::int_type(), make_type::text_type()]
}

#[test]
fn find_map_accepts_the_canonical_pair_mixop() {
    let value_map = map(vec![canonical_pair(integer(1, "key"), text("value"))]);
    let found = maps::find_map(
        &mut |_| {},
        &span("map-call"),
        &type_args(),
        &[value_map, integer(1, "lookup-key")],
    )
    .unwrap();

    assert_eq!(
        get::text(get::opt(&found).unwrap().expect("found value")).unwrap(),
        "value",
    );
}

#[test]
fn find_map_and_find_maps_return_the_first_semantic_match() {
    let callsite = span("map-call");
    let duplicate_map = map(vec![
        pair(integer(1, "first-key"), text("first")),
        pair(integer(1, "second-key"), text("second")),
    ]);
    let mut add = |_| {};
    let found = maps::find_map(
        &mut add,
        &callsite,
        &type_args(),
        &[duplicate_map, integer(1, "lookup-key")],
    )
    .unwrap();
    assert_eq!(
        get::text(get::opt(&found).unwrap().expect("found value")).unwrap(),
        "first",
    );

    let maps_value = make::list(
        &make_type::list_type(make_type::var_type(
            Spanned::new("map".to_owned(), Region::none()),
            type_args(),
        )),
        vec![
            map(vec![pair(integer(2, "two"), text("miss"))]),
            map(vec![pair(integer(1, "one"), text("later-map"))]),
        ],
        span("maps"),
    );
    let found = maps::find_maps(
        &mut add,
        &callsite,
        &type_args(),
        &[maps_value, integer(1, "lookup-key")],
    )
    .unwrap();
    assert_eq!(
        get::text(get::opt(&found).unwrap().expect("found value")).unwrap(),
        "later-map",
    );
}

#[test]
fn add_and_update_replace_only_the_first_match_or_append() {
    let callsite = span("map-call");
    let original = map(vec![
        pair(integer(1, "first"), text("old")),
        pair(integer(1, "duplicate"), text("duplicate")),
    ]);
    let mut recorded = Vec::new();
    let updated = {
        let mut add = |value| recorded.push(value);
        maps::add_map(
            &mut add,
            &callsite,
            &type_args(),
            &[original, integer(1, "lookup"), text("new")],
        )
        .unwrap()
    };
    assert_eq!(
        entries(&updated),
        vec![(1.into(), "new".into()), (1.into(), "duplicate".into())],
    );
    assert_eq!(recorded.len(), 3);

    let appended = maps::update_map(
        &mut |_| {},
        &callsite,
        &type_args(),
        &[updated, integer(2, "two"), text("two")],
    )
    .unwrap();
    assert_eq!(
        entries(&appended),
        vec![
            (1.into(), "new".into()),
            (1.into(), "duplicate".into()),
            (2.into(), "two".into()),
        ],
    );
}

#[test]
fn adds_map_updates_in_input_order_and_rejects_unequal_lists() {
    let callsite = span("map-call");
    let empty = map(Vec::new());
    let keys = make::list(
        &make_type::list_type(make_type::int_type()),
        vec![integer(1, "one"), integer(2, "two")],
        span("keys"),
    );
    let values = make::list(
        &make_type::list_type(make_type::text_type()),
        vec![text("one"), text("two")],
        span("values"),
    );
    let result = maps::adds_map(
        &mut |_| {},
        &callsite,
        &type_args(),
        &[empty.clone(), keys.clone(), values],
    )
    .unwrap();
    assert_eq!(
        entries(&result),
        vec![(1.into(), "one".into()), (2.into(), "two".into())],
    );

    let error = maps::adds_map(
        &mut |_| {},
        &callsite,
        &type_args(),
        &[
            empty,
            keys,
            make::list(
                &make_type::list_type(make_type::text_type()),
                vec![text("one")],
                span("short-values"),
            ),
        ],
    )
    .unwrap_err();
    assert_eq!(error.span, callsite);
    assert!(error.message.contains("same length"));
}

#[test]
fn malformed_map_is_a_typed_builtin_error() {
    let error = maps::find_map(
        &mut |_| {},
        &span("bad-call"),
        &type_args(),
        &[integer(0, "not-a-map"), integer(1, "key")],
    )
    .unwrap_err();
    assert_eq!(error.span, span("bad-call"));
    assert!(error.message.contains("expected a map"));
}
