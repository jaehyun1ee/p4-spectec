//! Runtime value-membership tests

use num_bigint::BigInt;
use p4spec_rust::lang::data::value::ValueArena;
use p4spec_rust::{
    lang::{
        common::source::Span,
        data::{typ, value::make},
        il::ast::{FuncTyp, Iter, Subcheck},
        xl::num::Natural,
    },
    phrase,
    runtime::{
        envs::elab::TDEnv,
        ops::value::{MatchError, check, sub, subs},
        typdef::TypeDef,
    },
};

fn id(name: &str) -> p4spec_rust::lang::il::ast::Id {
    phrase!(node: name.to_owned(), span: Span::default())
}

#[test]
fn test_numeric_membership_preserves_nat_subtyping() {
    let mut arena = ValueArena::new();
    let tdenv = TDEnv::new();
    let find_func = |_: &str| None::<FuncTyp>;
    let nat = make::nat(&mut arena, Natural::from(3_u64), Span::default()).unwrap();
    let nonnegative_int = make::int(&mut arena, BigInt::from(3), Span::default()).unwrap();
    let negative_int = make::int(&mut arena, BigInt::from(-1), Span::default()).unwrap();

    assert_eq!(
        sub(&arena, &tdenv, &find_func, &typ::make::nat(), &nat),
        Ok(true)
    );
    assert_eq!(
        sub(
            &arena,
            &tdenv,
            &find_func,
            &typ::make::nat(),
            &nonnegative_int
        ),
        Ok(true)
    );
    assert_eq!(
        sub(&arena, &tdenv, &find_func, &typ::make::nat(), &negative_int),
        Ok(false)
    );
    assert_eq!(
        sub(&arena, &tdenv, &find_func, &typ::make::int(), &nat),
        Ok(true)
    );
}

#[test]
fn test_extern_type_membership_uses_shared_type_environment() {
    let mut arena = ValueArena::new();
    let mut tdenv = TDEnv::new();
    let extern_id = id("object");
    tdenv.insert(extern_id.clone(), TypeDef::Extern);
    let extern_typ = typ::make::var(extern_id, vec![]);
    let value = make::external(
        &mut arena,
        (extern_typ).node.clone().into(),
        serde_json::Value::Null,
        Span::default(),
    )
    .unwrap();
    let find_func = |_: &str| None::<FuncTyp>;

    assert_eq!(
        sub(&arena, &tdenv, &find_func, &extern_typ, &value),
        Ok(true)
    );
}

#[test]
fn test_undefined_names_return_located_typed_errors() {
    let mut arena = ValueArena::new();
    let missing_typ = typ::make::var(id("missing"), vec![]);
    let value = make::bool(&mut arena, true, Span::default()).unwrap();

    let error = sub(
        &arena,
        &TDEnv::new(),
        &|_: &str| None::<FuncTyp>,
        &missing_typ,
        &value,
    )
    .unwrap_err();

    assert!(matches!(error, MatchError::UndefinedType { ref name, .. } if name == "missing"));

    let mut span = Span::default();
    span.left.line = 17;
    let value = make::func(
        &mut arena,
        p4spec_rust::phrase!(node: "missing".to_owned(), span: span.clone()),
        vec![],
        vec![],
        typ::make::bool(),
        Span::default(),
    )
    .unwrap();
    let typ = typ::make::func(vec![], vec![], typ::make::bool());
    let error = sub(&arena, &TDEnv::default(), &|_| None, &typ, &value).unwrap_err();
    assert!(
        matches!(error, MatchError::UndefinedFunction { name, span: span_error } if name == "missing" && span_error == span)
    );
}

#[test]
fn test_recursive_subchecks_walk_tuple_and_list_values() {
    let mut arena = ValueArena::new();
    let bool_typ = typ::make::bool();
    let tuple_typ = typ::make::tuple(vec![bool_typ.clone(), typ::make::list(bool_typ.clone())]);
    let bool_value = make::bool(&mut arena, true, Span::default()).unwrap();
    let list_value = {
        let values = vec![make::bool(&mut arena, false, Span::default()).unwrap()];
        make::list(
            &mut arena,
            (typ::make::list(bool_typ.clone())).node.clone().into(),
            values,
            Span::default(),
        )
        .unwrap()
    };
    let tuple_value = make::tuple(
        &mut arena,
        (tuple_typ).node.clone().into(),
        vec![bool_value, list_value],
        Span::default(),
    )
    .unwrap();
    let subcheck = Subcheck::Tuple(vec![
        Subcheck::Recurse(bool_typ.clone()),
        Subcheck::Iter(Iter::List, Box::new(Subcheck::Recurse(bool_typ))),
    ]);

    assert_eq!(
        check(
            &arena,
            &TDEnv::new(),
            &|_: &str| None::<FuncTyp>,
            &subcheck,
            &tuple_value
        ),
        Ok(true)
    );
}

#[test]
fn test_list_membership_rejects_arity_mismatch() {
    let mut arena = ValueArena::new();
    let values = vec![make::bool(&mut arena, true, Span::default()).unwrap()];
    let func_typ = FuncTyp {
        tparams: vec![],
        typs_params: vec![],
        typ_ret: Box::new(typ::make::bool()),
    };

    assert_eq!(
        subs(
            &arena,
            &TDEnv::new(),
            &|_: &str| Some(func_typ.clone()),
            &[typ::make::bool(), typ::make::bool()],
            &values
        ),
        Ok(false)
    );
}
