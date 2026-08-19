use num_bigint::BigInt;
use p4spec_rust::{
    domain::source::{Region, Spanned},
    interp::sl::{context::Context, expression},
    lang::{il::ast as il, xl::num},
    runtime::value::get,
};

fn span(file: &str) -> Region {
    Region::for_file(file)
}

fn numeric(value: num::T, file: &str) -> il::Exp {
    let typ = match value {
        num::T::Nat(_) => num::Typ::NatT,
        num::T::Int(_) => num::Typ::IntT,
    };
    il::Exp::new(il::ExpKind::NumE(value), il::TypKind::NumT(typ), span(file))
}

fn typ(kind: il::TypKind, file: &str) -> il::Typ {
    Spanned::new(kind, span(file))
}

#[test]
fn numeric_upcast_and_downcast_follow_nat_int_subtyping() {
    let mut context = Context::from_spec(false, &[]).unwrap();
    let upcast = il::Exp::new(
        il::ExpKind::UpCastE(
            typ(il::TypKind::NumT(num::Typ::IntT), "int-type"),
            Box::new(numeric(num::T::Nat(BigInt::from(3)), "nat")),
        ),
        il::TypKind::NumT(num::Typ::IntT),
        span("upcast"),
    );
    assert!(
        matches!(expression::eval(&mut context, &upcast).map(|v| v.kind.clone()), Ok(p4spec_rust::runtime::value::ValueKind::NumV(num::T::Int(value))) if value == BigInt::from(3))
    );

    let downcast = il::Exp::new(
        il::ExpKind::DownCastE(
            typ(il::TypKind::NumT(num::Typ::NatT), "nat-type"),
            Box::new(numeric(num::T::Int(BigInt::from(3)), "int")),
        ),
        il::TypKind::NumT(num::Typ::NatT),
        span("downcast"),
    );
    assert!(
        matches!(expression::eval(&mut context, &downcast).map(|v| v.kind.clone()), Ok(p4spec_rust::runtime::value::ValueKind::NumV(num::T::Nat(value))) if value == BigInt::from(3))
    );

    let negative = il::Exp::new(
        il::ExpKind::DownCastE(
            typ(il::TypKind::NumT(num::Typ::NatT), "nat-type"),
            Box::new(numeric(num::T::Int(BigInt::from(-1)), "negative")),
        ),
        il::TypKind::NumT(num::Typ::NatT),
        span("negative-downcast"),
    );
    let error = expression::eval(&mut context, &negative).unwrap_err();
    assert_eq!(error.span, span("nat-type"));
    assert!(error.message.contains("cannot downcast"));
}

#[test]
fn tuple_option_and_list_casts_recurse_into_children() {
    let mut context = Context::from_spec(false, &[]).unwrap();
    let int_type = typ(il::TypKind::NumT(num::Typ::IntT), "int-type");
    let target = typ(
        il::TypKind::TupleT(vec![
            int_type.clone(),
            typ(
                il::TypKind::IterT(Box::new(int_type.clone()), il::Iter::List),
                "list-type",
            ),
        ]),
        "tuple-type",
    );
    let source_list = il::Exp::new(
        il::ExpKind::ListE(vec![numeric(num::T::Nat(1.into()), "one")]),
        il::TypKind::IterT(
            Box::new(typ(il::TypKind::NumT(num::Typ::NatT), "nat-type")),
            il::Iter::List,
        ),
        span("list"),
    );
    let source = il::Exp::new(
        il::ExpKind::TupleE(vec![numeric(num::T::Nat(2.into()), "two"), source_list]),
        il::TypKind::TupleT(Vec::new()),
        span("tuple"),
    );
    let cast = il::Exp::new(
        il::ExpKind::UpCastE(target.clone(), Box::new(source)),
        target.node.clone(),
        span("cast"),
    );
    let value = expression::eval(&mut context, &cast).unwrap();
    let values = get::tuple(&value).unwrap();
    assert!(matches!(get::num(&values[0]), Ok(num::T::Int(value)) if value == &BigInt::from(2)));
    assert!(
        matches!(get::num(&get::list(&values[1]).unwrap()[0]), Ok(num::T::Int(value)) if value == &BigInt::from(1))
    );
}

#[test]
fn subtype_expression_checks_numeric_and_iterated_values() {
    let mut context = Context::from_spec(false, &[]).unwrap();
    let positive = il::Exp::new(
        il::ExpKind::SubE(
            Box::new(numeric(num::T::Int(2.into()), "positive")),
            typ(il::TypKind::NumT(num::Typ::NatT), "nat-type"),
        ),
        il::TypKind::BoolT,
        span("positive-sub"),
    );
    assert_eq!(
        get::bool(&expression::eval(&mut context, &positive).unwrap()),
        Ok(true)
    );
    let negative = il::Exp::new(
        il::ExpKind::SubE(
            Box::new(numeric(num::T::Int((-2).into()), "negative")),
            typ(il::TypKind::NumT(num::Typ::NatT), "nat-type"),
        ),
        il::TypKind::BoolT,
        span("negative-sub"),
    );
    assert_eq!(
        get::bool(&expression::eval(&mut context, &negative).unwrap()),
        Ok(false)
    );
}
