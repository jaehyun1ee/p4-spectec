use num_bigint::BigInt;
use p4spec_rust::{
    domain::source::{Region, Spanned},
    interp::sl::{context::Context, expression},
    lang::{il::ast as il, xl::num},
    runtime::{
        dynamic::var::Variable,
        value::{get, make},
    },
};

fn span(file: &str) -> Region {
    Region::for_file(file)
}

fn id(name: &str, file: &str) -> il::Id {
    Spanned::new(name.to_owned(), span(file))
}

fn bool_exp(value: bool, file: &str) -> il::Exp {
    il::Exp::new(il::ExpKind::BoolE(value), il::TypKind::BoolT, span(file))
}

fn nat_exp(value: i64, file: &str) -> il::Exp {
    il::Exp::new(
        il::ExpKind::NumE(num::T::Nat(BigInt::from(value))),
        il::TypKind::NumT(num::Typ::NatT),
        span(file),
    )
}

fn int_exp(value: i64, file: &str) -> il::Exp {
    il::Exp::new(
        il::ExpKind::NumE(num::T::Int(BigInt::from(value))),
        il::TypKind::NumT(num::Typ::IntT),
        span(file),
    )
}

fn eval(context: &Context, exp: &il::Exp) -> p4spec_rust::runtime::value::ValueRef {
    expression::eval(context, exp).unwrap()
}

#[test]
fn literals_and_variables_produce_runtime_values() {
    let mut context = Context::from_spec(false, &[]).unwrap();
    context.enter_relation(id("R", "relation"), Vec::new());
    let boolean = eval(&context, &bool_exp(true, "bool-exp"));
    assert_eq!(get::bool(&boolean), Ok(true));
    assert_eq!(boolean.span, Region::none());
    let number = eval(&context, &nat_exp(7, "num-exp"));
    assert!(matches!(get::num(&number), Ok(num::T::Nat(value)) if value == &BigInt::from(7)));
    let text_exp = il::Exp::new(
        il::ExpKind::TextE("hello".to_owned()),
        il::TypKind::TextT,
        span("text-exp"),
    );
    assert_eq!(get::text(&eval(&context, &text_exp)), Ok("hello"));

    let bound = make::bool(false, span("bound"));
    context
        .bind_value(Variable::new(id("x", "binding"), Vec::new()), bound.clone())
        .unwrap();
    let variable = il::Exp::new(
        il::ExpKind::VarE(id("x", "variable-exp")),
        il::TypKind::BoolT,
        span("variable-exp"),
    );
    assert!(std::rc::Rc::ptr_eq(&eval(&context, &variable), &bound));
}

#[test]
fn unary_and_boolean_binary_operations_match_ocaml() {
    let context = Context::from_spec(false, &[]).unwrap();
    let not = il::Exp::new(
        il::ExpKind::UnE(
            il::UnOp::NotOp,
            il::OpTyp::BoolT,
            Box::new(bool_exp(true, "arg")),
        ),
        il::TypKind::BoolT,
        span("not"),
    );
    assert_eq!(get::bool(&eval(&context, &not)), Ok(false));
    let minus = il::Exp::new(
        il::ExpKind::UnE(
            il::UnOp::MinusOp,
            il::OpTyp::NatT,
            Box::new(nat_exp(3, "arg")),
        ),
        il::TypKind::NumT(num::Typ::IntT),
        span("minus"),
    );
    assert!(
        matches!(get::num(&eval(&context, &minus)), Ok(num::T::Int(value)) if value == &BigInt::from(-3))
    );

    for (operator, expected) in [
        (il::BinOp::AndOp, false),
        (il::BinOp::OrOp, true),
        (il::BinOp::ImplOp, true),
        (il::BinOp::EquivOp, false),
    ] {
        let exp = il::Exp::new(
            il::ExpKind::BinE(
                operator,
                il::OpTyp::BoolT,
                Box::new(bool_exp(false, "left")),
                Box::new(bool_exp(true, "right")),
            ),
            il::TypKind::BoolT,
            span("binary"),
        );
        assert_eq!(get::bool(&eval(&context, &exp)), Ok(expected));
    }
}

#[test]
fn numeric_binary_and_comparison_operations_preserve_number_kinds() {
    let context = Context::from_spec(false, &[]).unwrap();
    let subtraction = il::Exp::new(
        il::ExpKind::BinE(
            il::BinOp::SubOp,
            il::OpTyp::NatT,
            Box::new(nat_exp(2, "left")),
            Box::new(nat_exp(5, "right")),
        ),
        il::TypKind::NumT(num::Typ::IntT),
        span("sub"),
    );
    assert!(
        matches!(get::num(&eval(&context, &subtraction)), Ok(num::T::Int(value)) if value == &BigInt::from(-3))
    );
    let division = il::Exp::new(
        il::ExpKind::BinE(
            il::BinOp::DivOp,
            il::OpTyp::IntT,
            Box::new(int_exp(-7, "left")),
            Box::new(int_exp(2, "right")),
        ),
        il::TypKind::NumT(num::Typ::IntT),
        span("div"),
    );
    assert!(
        matches!(get::num(&eval(&context, &division)), Ok(num::T::Int(value)) if value == &BigInt::from(-3))
    );

    let comparison = il::Exp::new(
        il::ExpKind::CmpE(
            il::CmpOp::LtOp,
            il::OpTyp::IntT,
            Box::new(int_exp(-1, "left")),
            Box::new(int_exp(2, "right")),
        ),
        il::TypKind::BoolT,
        span("comparison"),
    );
    assert_eq!(get::bool(&eval(&context, &comparison)), Ok(true));
}

#[test]
fn equality_is_semantic_and_boolean_binary_evaluation_is_eager() {
    let context = Context::from_spec(false, &[]).unwrap();
    let equality = il::Exp::new(
        il::ExpKind::CmpE(
            il::CmpOp::EqOp,
            il::OpTyp::IntT,
            Box::new(int_exp(1, "left-span")),
            Box::new(int_exp(1, "right-span")),
        ),
        il::TypKind::BoolT,
        span("equality"),
    );
    assert_eq!(get::bool(&eval(&context, &equality)), Ok(true));

    let missing = il::Exp::new(
        il::ExpKind::VarE(id("missing", "missing")),
        il::TypKind::BoolT,
        span("missing"),
    );
    let eager = il::Exp::new(
        il::ExpKind::BinE(
            il::BinOp::AndOp,
            il::OpTyp::BoolT,
            Box::new(bool_exp(false, "left")),
            Box::new(missing),
        ),
        il::TypKind::BoolT,
        span("eager"),
    );
    let error = expression::eval(&context, &eager).unwrap_err();
    assert!(error.message.contains("value `missing` is undefined"));
}

#[test]
fn invalid_scalar_operations_return_typed_errors() {
    let context = Context::from_spec(false, &[]).unwrap();
    let division = il::Exp::new(
        il::ExpKind::BinE(
            il::BinOp::DivOp,
            il::OpTyp::NatT,
            Box::new(nat_exp(1, "left")),
            Box::new(nat_exp(0, "right")),
        ),
        il::TypKind::NumT(num::Typ::NatT),
        span("division"),
    );
    let error = expression::eval(&context, &division).unwrap_err();
    assert_eq!(error.span, span("division"));
    assert!(error.message.contains("division by zero"));

    let unsupported = il::Exp::new(
        il::ExpKind::UpCastE(
            Spanned::new(il::TypKind::BoolT, span("cast-type")),
            Box::new(bool_exp(true, "cast-value")),
        ),
        il::TypKind::BoolT,
        span("cast"),
    );
    let error = expression::eval(&context, &unsupported).unwrap_err();
    assert!(error.message.contains("not implemented"));
}
