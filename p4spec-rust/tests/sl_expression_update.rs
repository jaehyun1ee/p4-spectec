use std::rc::Rc;

use num_bigint::BigInt;
use p4spec_rust::{
    domain::{
        atom::Atom,
        source::{Region, Spanned},
    },
    interp::sl::{context::Context, expression},
    lang::{il::ast as il, xl::num},
    runtime::{
        dynamic::var::Variable,
        r#type::typ::make as make_type,
        value::{get, make},
    },
};

fn span(file: &str) -> Region {
    Region::for_file(file)
}

fn id(name: &str) -> il::Id {
    Spanned::new(name.to_owned(), span(name))
}

fn atom(name: &str) -> il::Atom {
    Spanned::new(Atom::Keyword(name.to_owned()), span(name))
}

fn variable(name: &str, ty: il::TypKind) -> il::Exp {
    il::Exp::new(il::ExpKind::VarE(id(name)), ty, span(name))
}

fn int_exp(value: i64, file: &str) -> il::Exp {
    il::Exp::new(
        il::ExpKind::NumE(num::T::Int(BigInt::from(value))),
        il::TypKind::NumT(num::Typ::IntT),
        span(file),
    )
}

fn text_exp(value: &str, file: &str) -> il::Exp {
    il::Exp::new(
        il::ExpKind::TextE(value.to_owned()),
        il::TypKind::TextT,
        span(file),
    )
}

fn root(ty: il::TypKind) -> il::Path {
    il::Path::new(il::PathKind::RootP, ty, span("root"))
}

fn context() -> Context {
    let mut context = Context::from_spec(false, &[]).unwrap();
    context.enter_relation(id("R"), Vec::new());
    context
}

#[test]
fn nested_update_rebuilds_the_path_and_shares_unaffected_values() {
    let mut context = context();
    let bool_type = make_type::bool_type();
    let list_type = make_type::list_type(bool_type.clone());
    let record_type = Spanned::new(
        il::TypKind::VarT(id("Record"), Vec::new()),
        span("record-type"),
    );
    let keep = make::bool(false, span("keep"));
    let old = make::bool(false, span("old"));
    let sibling = make::text("same".to_owned(), span("sibling"));
    let replacement = make::bool(true, span("replacement"));
    let list = make::list(
        &list_type,
        vec![keep.clone(), old.clone()],
        span("list-value"),
    );
    let base = make::structure(
        &record_type,
        vec![
            (atom("items"), list.clone()),
            (atom("sibling"), sibling.clone()),
        ],
        span("base-value"),
    );
    context
        .bind_value(Variable::new(id("base"), Vec::new()), base.clone())
        .unwrap();
    context
        .bind_value(
            Variable::new(id("replacement"), Vec::new()),
            replacement.clone(),
        )
        .unwrap();

    let items_path = il::Path::new(
        il::PathKind::DotP(Box::new(root(record_type.node.clone())), atom("items")),
        list_type.node.clone(),
        span("items-path"),
    );
    let item_path = il::Path::new(
        il::PathKind::IdxP(Box::new(items_path), Box::new(int_exp(1, "item-index"))),
        bool_type.node.clone(),
        span("item-path"),
    );
    let update = il::Exp::new(
        il::ExpKind::UpdE(
            Box::new(variable("base", record_type.node.clone())),
            item_path,
            Box::new(variable("replacement", bool_type.node.clone())),
        ),
        record_type.node.clone(),
        span("update"),
    );

    let result = expression::eval(&context, &update).unwrap();
    let result_fields = get::structure(&result).unwrap();
    let result_items = get::list(&result_fields[0].1).unwrap();
    assert!(!Rc::ptr_eq(&result, &base));
    assert!(!Rc::ptr_eq(&result_fields[0].1, &list));
    assert!(Rc::ptr_eq(&result_items[0], &keep));
    assert!(Rc::ptr_eq(&result_items[1], &replacement));
    assert!(Rc::ptr_eq(&result_fields[1].1, &sibling));
    assert!(Rc::ptr_eq(&get::list(&list).unwrap()[1], &old));
    assert_eq!(result.ty, record_type.node);
    assert_eq!(result_fields[0].1.ty, list_type.node);
}

#[test]
fn text_index_and_slice_updates_match_ocaml_fixed_width_replacement() {
    let context = context();
    let text_type = il::TypKind::TextT;
    let indexed = il::Exp::new(
        il::ExpKind::UpdE(
            Box::new(text_exp("abc", "index-base")),
            il::Path::new(
                il::PathKind::IdxP(
                    Box::new(root(text_type.clone())),
                    Box::new(int_exp(1, "index")),
                ),
                text_type.clone(),
                span("index-path"),
            ),
            Box::new(text_exp("Z", "index-replacement")),
        ),
        text_type.clone(),
        span("index-update"),
    );
    assert_eq!(
        get::text(&expression::eval(&context, &indexed).unwrap()),
        Ok("aZc")
    );

    let sliced = il::Exp::new(
        il::ExpKind::UpdE(
            Box::new(text_exp("abcdef", "slice-base")),
            il::Path::new(
                il::PathKind::SliceP(
                    Box::new(root(text_type.clone())),
                    Box::new(int_exp(2, "slice-index")),
                    Box::new(int_exp(3, "slice-count")),
                ),
                text_type.clone(),
                span("slice-path"),
            ),
            Box::new(text_exp("XYZ", "slice-replacement")),
        ),
        text_type,
        span("slice-update"),
    );
    assert_eq!(
        get::text(&expression::eval(&context, &sliced).unwrap()),
        Ok("abXYZf")
    );
}

#[test]
fn list_slice_update_shares_values_outside_the_replaced_range() {
    let mut context = context();
    let int_type = make_type::int_type();
    let list_type = make_type::list_type(int_type);
    let first = make::int(0.into(), span("first"));
    let removed_a = make::int(1.into(), span("removed-a"));
    let removed_b = make::int(2.into(), span("removed-b"));
    let last = make::int(3.into(), span("last"));
    let replacement_a = make::int(8.into(), span("replacement-a"));
    let replacement_b = make::int(9.into(), span("replacement-b"));
    let base = make::list(
        &list_type,
        vec![first.clone(), removed_a, removed_b, last.clone()],
        span("base"),
    );
    let replacement = make::list(
        &list_type,
        vec![replacement_a.clone(), replacement_b.clone()],
        span("replacement"),
    );
    context
        .bind_value(Variable::new(id("base"), Vec::new()), base)
        .unwrap();
    context
        .bind_value(Variable::new(id("replacement"), Vec::new()), replacement)
        .unwrap();
    let update = il::Exp::new(
        il::ExpKind::UpdE(
            Box::new(variable("base", list_type.node.clone())),
            il::Path::new(
                il::PathKind::SliceP(
                    Box::new(root(list_type.node.clone())),
                    Box::new(int_exp(1, "slice-index")),
                    Box::new(int_exp(2, "slice-count")),
                ),
                list_type.node.clone(),
                span("slice-path"),
            ),
            Box::new(variable("replacement", list_type.node.clone())),
        ),
        list_type.node,
        span("update"),
    );

    let result = expression::eval(&context, &update).unwrap();
    let values = get::list(&result).unwrap();
    assert!(Rc::ptr_eq(&values[0], &first));
    assert!(Rc::ptr_eq(&values[1], &replacement_a));
    assert!(Rc::ptr_eq(&values[2], &replacement_b));
    assert!(Rc::ptr_eq(&values[3], &last));
}

#[test]
fn update_reports_bounds_and_replacement_width_errors_at_the_path_operand() {
    let context = context();
    let out_of_bounds = il::Exp::new(
        il::ExpKind::UpdE(
            Box::new(text_exp("a", "base")),
            il::Path::new(
                il::PathKind::IdxP(
                    Box::new(root(il::TypKind::TextT)),
                    Box::new(int_exp(2, "bad-index")),
                ),
                il::TypKind::TextT,
                span("index-path"),
            ),
            Box::new(text_exp("x", "replacement")),
        ),
        il::TypKind::TextT,
        span("update"),
    );
    let error = expression::eval(&context, &out_of_bounds).unwrap_err();
    assert!(error.message.contains("out of bounds"));
    assert_eq!(error.span, span("bad-index"));

    let wrong_width = il::Exp::new(
        il::ExpKind::UpdE(
            Box::new(text_exp("abc", "base")),
            il::Path::new(
                il::PathKind::SliceP(
                    Box::new(root(il::TypKind::TextT)),
                    Box::new(int_exp(0, "slice-index")),
                    Box::new(int_exp(2, "slice-count")),
                ),
                il::TypKind::TextT,
                span("slice-path"),
            ),
            Box::new(text_exp("x", "replacement")),
        ),
        il::TypKind::TextT,
        span("update"),
    );
    let error = expression::eval(&context, &wrong_width).unwrap_err();
    assert!(
        error
            .message
            .contains("slice of length 2 requires a text of length 1")
    );
    assert_eq!(error.span, span("slice-count"));
}
