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
    runtime::value::{ValueKind as RuntimeValueKind, get},
    wire::runtime_value::{to_canonical, to_runtime},
};

fn span(file: &str) -> Region {
    Region::for_file(file)
}

fn id(name: &str, file: &str) -> il::Id {
    Spanned::new(name.to_owned(), span(file))
}

fn atom(name: &str, file: &str) -> il::Atom {
    Spanned::new(Atom::Keyword(name.to_owned()), span(file))
}

fn value(kind: il::ValueKind, ty: TypKind, file: &str) -> il::Value {
    il::Value::new(kind, ty, span(file))
}

#[test]
fn canonical_and_runtime_values_round_trip_every_variant_exactly() {
    let bool_value = value(il::ValueKind::BoolV(true), TypKind::BoolT, "bool");
    let nat_value = value(
        il::ValueKind::NumV(num::T::Nat(7.into())),
        TypKind::NumT(num::Typ::NatT),
        "nat",
    );
    let int_value = value(
        il::ValueKind::NumV(num::T::Int((-3).into())),
        TypKind::NumT(num::Typ::IntT),
        "int",
    );
    let text_value = value(
        il::ValueKind::TextV("text".to_owned()),
        TypKind::TextT,
        "text",
    );
    let named_type = TypKind::VarT(id("Node", "type"), Vec::new());
    let values = vec![
        bool_value.clone(),
        nat_value.clone(),
        int_value,
        text_value.clone(),
        value(
            il::ValueKind::StructV(vec![(atom("field", "field"), bool_value.clone())]),
            named_type.clone(),
            "struct",
        ),
        value(
            il::ValueKind::CaseV(Box::new(Mixfix::Seq(vec![
                Mixfix::Atom(atom("SOME", "case-atom")),
                Mixfix::Arg(text_value.clone()),
            ]))),
            named_type.clone(),
            "case",
        ),
        value(
            il::ValueKind::TupleV(vec![bool_value.clone(), nat_value.clone()]),
            TypKind::TupleT(Vec::new()),
            "tuple",
        ),
        value(
            il::ValueKind::OptV(Some(Box::new(text_value.clone()))),
            TypKind::IterT(
                Box::new(Spanned::new(TypKind::TextT, span("opt-inner"))),
                il::Iter::Opt,
            ),
            "some",
        ),
        value(
            il::ValueKind::OptV(None),
            TypKind::IterT(
                Box::new(Spanned::new(TypKind::TextT, span("none-inner"))),
                il::Iter::Opt,
            ),
            "none",
        ),
        value(
            il::ValueKind::ListV(vec![bool_value, nat_value]),
            TypKind::IterT(
                Box::new(Spanned::new(TypKind::BoolT, span("list-inner"))),
                il::Iter::List,
            ),
            "list",
        ),
        value(
            il::ValueKind::FuncV(id("function", "function-id")),
            TypKind::FuncT(
                Vec::new(),
                Vec::new(),
                Box::new(Spanned::new(TypKind::BoolT, span("return"))),
            ),
            "function",
        ),
        value(
            il::ValueKind::ExternV(ExternalData::Assoc(vec![
                ("left".to_owned(), ExternalData::Int(1)),
                (
                    "right".to_owned(),
                    ExternalData::Variant(
                        "Some".to_owned(),
                        Some(Box::new(ExternalData::String("x".to_owned()))),
                    ),
                ),
            ])),
            named_type,
            "external",
        ),
    ];
    let root = value(
        il::ValueKind::ListV(values),
        TypKind::IterT(
            Box::new(Spanned::new(TypKind::BoolT, span("root-inner"))),
            il::Iter::List,
        ),
        "root",
    );

    let runtime = to_runtime(&root);
    let canonical = to_canonical(&runtime);

    assert_eq!(canonical, root);
    assert_eq!(runtime.ty, root.ty);
    assert_eq!(runtime.span, root.span);
    let RuntimeValueKind::ListV(runtime_values) = &runtime.kind else {
        panic!("expected runtime list");
    };
    assert!(matches!(
        runtime_values[0].kind,
        RuntimeValueKind::BoolV(true)
    ));
    assert!(matches!(
        runtime_values[4].kind,
        RuntimeValueKind::StructV(_)
    ));
    assert!(matches!(runtime_values[5].kind, RuntimeValueKind::CaseV(_)));
    assert!(matches!(
        runtime_values[10].kind,
        RuntimeValueKind::FuncV(_)
    ));
    assert!(matches!(
        runtime_values[11].kind,
        RuntimeValueKind::ExternV(_)
    ));
}

#[test]
fn bridge_preserves_nested_child_metadata_in_both_directions() {
    let child = value(
        il::ValueKind::TextV("child".to_owned()),
        TypKind::BoolT,
        "child-region",
    );
    let parent = value(
        il::ValueKind::OptV(Some(Box::new(child.clone()))),
        TypKind::TextT,
        "parent-region",
    );

    let runtime = to_runtime(&parent);
    let runtime_child = get::opt(&runtime)
        .expect("runtime option")
        .expect("runtime child");
    assert_eq!(runtime_child.ty, child.ty);
    assert_eq!(runtime_child.span, child.span);

    let canonical = to_canonical(&runtime);
    assert_eq!(canonical, parent);
}
