use num_bigint::BigInt;
use p4spec_rust::{
    domain::{
        atom::Atom,
        mixfix::{Mixfix, Mixop},
        source::{Region, Spanned},
    },
    interp::sl::{context::Context, expression},
    lang::{il::ast as il, xl::num},
    runtime::value::{ValueRef, get},
};

fn span(file: &str) -> Region {
    Region::for_file(file)
}

fn bool_exp(value: bool) -> il::Exp {
    il::Exp::new(il::ExpKind::BoolE(value), il::TypKind::BoolT, span("bool"))
}

fn int_exp(value: i64) -> il::Exp {
    il::Exp::new(
        il::ExpKind::NumE(num::T::Int(BigInt::from(value))),
        il::TypKind::NumT(num::Typ::IntT),
        span("int"),
    )
}

fn text_exp(value: &str) -> il::Exp {
    il::Exp::new(
        il::ExpKind::TextE(value.to_owned()),
        il::TypKind::TextT,
        span("text"),
    )
}

fn list_exp(values: Vec<il::Exp>) -> il::Exp {
    il::Exp::new(
        il::ExpKind::ListE(values),
        il::TypKind::IterT(
            Box::new(Spanned::new(
                il::TypKind::NumT(num::Typ::IntT),
                Region::none(),
            )),
            il::Iter::List,
        ),
        span("list"),
    )
}

fn eval(exp: &il::Exp) -> ValueRef {
    expression::eval(&Context::from_spec(false, &[]).unwrap(), exp).unwrap()
}

fn int_value(value: &ValueRef) -> BigInt {
    match get::num(value).unwrap() {
        num::T::Int(value) => value.clone(),
        num::T::Nat(_) => panic!("expected int"),
    }
}

#[test]
fn tuple_case_struct_option_and_list_preserve_ast_shape_and_type() {
    let tuple = il::Exp::new(
        il::ExpKind::TupleE(vec![bool_exp(true), text_exp("x")]),
        il::TypKind::TupleT(vec![
            Spanned::new(il::TypKind::BoolT, Region::none()),
            Spanned::new(il::TypKind::TextT, Region::none()),
        ]),
        span("tuple"),
    );
    let tuple_value = eval(&tuple);
    assert_eq!(get::tuple(&tuple_value).unwrap().len(), 2);
    assert_eq!(tuple_value.ty, tuple.ty);

    let case_mixop = Mixfix::Seq(vec![
        Mixfix::Atom(Spanned::new(Atom::Tag("SOME".to_owned()), Region::none())),
        Mixfix::Arg(()),
    ]);
    let case_exp = il::Exp::new(
        il::ExpKind::CaseE(Box::new(Mixop::fill(&case_mixop, [int_exp(4)]).unwrap())),
        il::TypKind::VarT(
            Spanned::new("caseType".to_owned(), Region::none()),
            Vec::new(),
        ),
        span("case"),
    );
    let case_value = eval(&case_exp);
    assert_eq!(case_value.ty, case_exp.ty);
    assert_eq!(get::case(&case_value).unwrap().split().0, case_mixop);

    let field = Spanned::new(Atom::Keyword("field".to_owned()), span("field"));
    let structure = il::Exp::new(
        il::ExpKind::StrE(vec![(field.clone(), bool_exp(false))]),
        il::TypKind::VarT(
            Spanned::new("record".to_owned(), Region::none()),
            Vec::new(),
        ),
        span("structure"),
    );
    let structure_value = eval(&structure);
    assert_eq!(get::structure(&structure_value).unwrap()[0].0, field);

    let option = il::Exp::new(
        il::ExpKind::OptE(Some(Box::new(int_exp(1)))),
        il::TypKind::IterT(
            Box::new(Spanned::new(
                il::TypKind::NumT(num::Typ::IntT),
                Region::none(),
            )),
            il::Iter::Opt,
        ),
        span("option"),
    );
    assert!(get::opt(&eval(&option)).unwrap().is_some());
    assert_eq!(
        get::list(&eval(&list_exp(vec![int_exp(1), int_exp(2)])))
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn cons_and_concatenation_handle_lists_and_text() {
    let cons = il::Exp::new(
        il::ExpKind::ConsE(
            Box::new(int_exp(0)),
            Box::new(list_exp(vec![int_exp(1), int_exp(2)])),
        ),
        list_exp(Vec::new()).ty,
        span("cons"),
    );
    let values = get::list(&eval(&cons))
        .unwrap()
        .iter()
        .map(int_value)
        .collect::<Vec<_>>();
    assert_eq!(values, vec![0.into(), 1.into(), 2.into()]);

    let list_cat = il::Exp::new(
        il::ExpKind::CatE(
            Box::new(list_exp(vec![int_exp(1)])),
            Box::new(list_exp(vec![int_exp(2)])),
        ),
        list_exp(Vec::new()).ty,
        span("list-cat"),
    );
    assert_eq!(get::list(&eval(&list_cat)).unwrap().len(), 2);
    let text_cat = il::Exp::new(
        il::ExpKind::CatE(Box::new(text_exp("ab")), Box::new(text_exp("cd"))),
        il::TypKind::TextT,
        span("text-cat"),
    );
    assert_eq!(get::text(&eval(&text_cat)), Ok("abcd"));
}

#[test]
fn membership_length_dot_index_and_slice_match_ocaml() {
    let membership = il::Exp::new(
        il::ExpKind::MemE(
            Box::new(int_exp(2)),
            Box::new(list_exp(vec![int_exp(1), int_exp(2)])),
        ),
        il::TypKind::BoolT,
        span("membership"),
    );
    assert_eq!(get::bool(&eval(&membership)), Ok(true));
    let length = il::Exp::new(
        il::ExpKind::LenE(Box::new(text_exp("abc"))),
        il::TypKind::NumT(num::Typ::NatT),
        span("length"),
    );
    assert!(
        matches!(get::num(&eval(&length)), Ok(num::T::Nat(value)) if value == &BigInt::from(3))
    );

    let field = Spanned::new(Atom::Keyword("field".to_owned()), span("field"));
    let structure = il::Exp::new(
        il::ExpKind::StrE(vec![(field.clone(), text_exp("value"))]),
        il::TypKind::VarT(
            Spanned::new("record".to_owned(), Region::none()),
            Vec::new(),
        ),
        span("structure"),
    );
    let dot = il::Exp::new(
        il::ExpKind::DotE(Box::new(structure), field),
        il::TypKind::TextT,
        span("dot"),
    );
    assert_eq!(get::text(&eval(&dot)), Ok("value"));

    let index = il::Exp::new(
        il::ExpKind::IdxE(Box::new(text_exp("abc")), Box::new(int_exp(1))),
        il::TypKind::TextT,
        span("index"),
    );
    assert_eq!(get::text(&eval(&index)), Ok("b"));
    let slice = il::Exp::new(
        il::ExpKind::SliceE(
            Box::new(list_exp(vec![int_exp(1), int_exp(2), int_exp(3)])),
            Box::new(int_exp(1)),
            Box::new(int_exp(2)),
        ),
        list_exp(Vec::new()).ty,
        span("slice"),
    );
    assert_eq!(
        get::list(&eval(&slice))
            .unwrap()
            .iter()
            .map(int_value)
            .collect::<Vec<_>>(),
        vec![2.into(), 3.into()],
    );
}

#[test]
fn pattern_matching_covers_case_list_and_option_patterns() {
    let case_mixop: Mixop =
        Mixfix::Atom(Spanned::new(Atom::Tag("EMPTY".to_owned()), Region::none()));
    let case = il::Exp::new(
        il::ExpKind::CaseE(Box::new(Mixop::fill(&case_mixop, []).unwrap())),
        il::TypKind::VarT(Spanned::new("case".to_owned(), Region::none()), Vec::new()),
        span("case"),
    );
    let case_match = il::Exp::new(
        il::ExpKind::MatchE(Box::new(case), il::Pattern::CaseP(case_mixop)),
        il::TypKind::BoolT,
        span("case-match"),
    );
    assert_eq!(get::bool(&eval(&case_match)), Ok(true));

    for (pattern, expected) in [
        (il::Pattern::ListP(il::ListPattern::Cons), true),
        (il::Pattern::ListP(il::ListPattern::Fixed(2)), true),
        (il::Pattern::ListP(il::ListPattern::Nil), false),
    ] {
        let matched = il::Exp::new(
            il::ExpKind::MatchE(Box::new(list_exp(vec![int_exp(1), int_exp(2)])), pattern),
            il::TypKind::BoolT,
            span("list-match"),
        );
        assert_eq!(get::bool(&eval(&matched)), Ok(expected));
    }
    let none = il::Exp::new(
        il::ExpKind::OptE(None),
        il::TypKind::IterT(
            Box::new(Spanned::new(il::TypKind::BoolT, Region::none())),
            il::Iter::Opt,
        ),
        span("none"),
    );
    let none_match = il::Exp::new(
        il::ExpKind::MatchE(Box::new(none), il::Pattern::OptP(il::OptPattern::None)),
        il::TypKind::BoolT,
        span("none-match"),
    );
    assert_eq!(get::bool(&eval(&none_match)), Ok(true));
}

#[test]
fn structural_type_and_bounds_failures_are_typed_errors() {
    let bad_index = il::Exp::new(
        il::ExpKind::IdxE(Box::new(text_exp("a")), Box::new(int_exp(2))),
        il::TypKind::TextT,
        span("bad-index"),
    );
    let error = expression::eval(&Context::from_spec(false, &[]).unwrap(), &bad_index).unwrap_err();
    assert!(error.message.contains("out of bounds"));

    let bad_cat = il::Exp::new(
        il::ExpKind::CatE(Box::new(text_exp("a")), Box::new(list_exp(Vec::new()))),
        il::TypKind::TextT,
        span("bad-cat"),
    );
    let error = expression::eval(&Context::from_spec(false, &[]).unwrap(), &bad_cat).unwrap_err();
    assert!(error.message.contains("concatenation expects"));
}
