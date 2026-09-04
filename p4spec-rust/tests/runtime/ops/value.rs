//! Runtime value-membership tests

use num_bigint::BigInt;
use p4spec_rust::{
    lang::{
        common::source::Span,
        data::{typ, value::make},
        il::ast::{FuncTyp, Iter, Subcheck},
        xl::num::Natural,
    },
    phrase,
    runtime::{
        env::TDEnv,
        ops::value::{MatchError, check, sub, subs},
        typdef::TypeDef,
    },
};

fn id(name: &str) -> p4spec_rust::lang::il::ast::Id {
    phrase!(node: name.to_owned(), span: Span::default())
}

#[test]
fn test_numeric_membership_preserves_nat_subtyping() {
    let tdenv = TDEnv::new();
    let find_func = |_: &str| None::<FuncTyp>;
    let natural = make::nat(Natural::from(3_u64), Span::default());
    let nonnegative_int = make::int(BigInt::from(3), Span::default());
    let negative_int = make::int(BigInt::from(-1), Span::default());

    assert_eq!(
        sub(&tdenv, &find_func, &typ::make::nat(), &natural),
        Ok(true)
    );
    assert_eq!(
        sub(&tdenv, &find_func, &typ::make::nat(), &nonnegative_int),
        Ok(true)
    );
    assert_eq!(
        sub(&tdenv, &find_func, &typ::make::nat(), &negative_int),
        Ok(false)
    );
    assert_eq!(
        sub(&tdenv, &find_func, &typ::make::int(), &natural),
        Ok(true)
    );
}

#[test]
fn test_extern_type_membership_uses_shared_type_environment() {
    let mut tdenv = TDEnv::new();
    let extern_id = id("object");
    tdenv.insert(extern_id.clone(), TypeDef::Extern);
    let extern_typ = typ::make::var(extern_id, vec![]);
    let value = make::external(
        &extern_typ,
        p4spec_rust::yojson::ExternalData::Null,
        Span::default(),
    );
    let find_func = |_: &str| None::<FuncTyp>;

    assert_eq!(sub(&tdenv, &find_func, &extern_typ, &value), Ok(true));
}

#[test]
fn test_undefined_types_return_located_typed_errors() {
    let missing_typ = typ::make::var(id("missing"), vec![]);
    let value = make::bool(true, Span::default());

    let error = sub(
        &TDEnv::new(),
        &|_: &str| None::<FuncTyp>,
        &missing_typ,
        &value,
    )
    .unwrap_err();

    assert!(matches!(error, MatchError::UndefinedType { ref name, .. } if name == "missing"));
}

#[test]
fn test_recursive_subchecks_walk_tuple_and_list_values() {
    let bool_typ = typ::make::bool();
    let tuple_typ = typ::make::tuple(vec![bool_typ.clone(), typ::make::list(bool_typ.clone())]);
    let bool_value = make::bool(true, Span::default());
    let list_value = make::list(
        &typ::make::list(bool_typ.clone()),
        vec![make::bool(false, Span::default())],
        Span::default(),
    );
    let tuple_value = make::tuple(&tuple_typ, vec![bool_value, list_value], Span::default());
    let subcheck = Subcheck::Tuple(vec![
        Subcheck::Recurse(bool_typ.clone()),
        Subcheck::Iter(Iter::List, Box::new(Subcheck::Recurse(bool_typ))),
    ]);

    assert_eq!(
        check(
            &TDEnv::new(),
            &|_: &str| None::<FuncTyp>,
            &subcheck,
            &tuple_value,
        ),
        Ok(true)
    );
}

#[test]
fn test_list_membership_rejects_arity_mismatch() {
    let values = vec![make::bool(true, Span::default())];
    let func_typ = FuncTyp {
        tparams: vec![],
        typs_params: vec![],
        typ_ret: Box::new(typ::make::bool()),
    };

    assert_eq!(
        subs(
            &TDEnv::new(),
            &|_: &str| Some(func_typ.clone()),
            &[typ::make::bool(), typ::make::bool()],
            &values,
        ),
        Ok(false)
    );
}
