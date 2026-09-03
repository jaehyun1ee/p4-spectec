use std::rc::Rc;

use p4spec_rust::{
    interface::p4_unparse::{P4UnparseError, P4Unparser},
    lang::{
        al,
        common::{
            notation::{atom::Atom, mixfix::Mixfix},
            source::Span,
        },
        el, il, pl, sl,
        xl::num::Natural,
    },
    runtime::{
        types::typ,
        value::{Value, make},
    },
};

fn id(name: &str) -> il::ast::Id {
    p4spec_rust::phrase! { node: name.to_owned(), span: Span::default() }
}

fn atom(name: &str) -> il::ast::Atom {
    p4spec_rust::phrase! {
        node: Atom::Keyword(name.to_owned()),
        span: Span::default(),
    }
}

fn hinted_def_type() -> il::ast::DefTyp {
    let notation = Mixfix::Seq(vec![Mixfix::Atom(atom("WRAP")), Mixfix::Arg(typ::text())]);
    let hint = p4spec_rust::phrase! {
        node: el::ast::ExpKind::Seq(vec![
            p4spec_rust::phrase! {
                node: el::ast::ExpKind::Text("show".to_owned()),
                span: Span::default(),
            },
            p4spec_rust::phrase! {
                node: el::ast::ExpKind::Hole(el::ast::Hole::Next),
                span: Span::default(),
            },
        ]),
        span: Span::default(),
    };
    p4spec_rust::phrase! {
        node: il::ast::DefTypKind::Variant(vec![(
            p4spec_rust::phrase! { node: notation, span: Span::default() },
            p4spec_rust::phrase! {
                node: (id("Origin"), Vec::new()),
                span: Span::default(),
            },
            vec![(id("print"), hint)],
        )]),
        span: Span::default(),
    }
}

fn wrapped_text() -> Rc<Value> {
    let wrapper_type = typ::var(id("Wrapper"), Vec::new());
    let value_case = Mixfix::Seq(vec![
        Mixfix::Atom(atom("WRAP")),
        Mixfix::Arg(make::text("payload".to_owned(), Span::default())),
    ]);
    make::case(&wrapper_type, value_case, Span::default())
}

#[test]
fn test_unparses_scalar_and_container_values() {
    let unparser = P4Unparser::new();
    let span = Span::default();
    assert_eq!(
        unparser.render(&make::bool(true, span.clone())).unwrap(),
        "true"
    );
    assert_eq!(
        unparser
            .render(&make::nat(Natural::from(42_u64), span.clone()))
            .unwrap(),
        "42"
    );
    assert_eq!(
        unparser
            .render(&make::text("a\n\"b".into(), span.clone()))
            .unwrap(),
        "a\\n\\\"b"
    );

    let tuple_type = typ::tuple(vec![typ::bool(), typ::nat()]);
    let tuple = make::tuple(
        &tuple_type,
        vec![
            make::bool(false, span.clone()),
            make::nat(7_u64.into(), span.clone()),
        ],
        span,
    );
    assert_eq!(unparser.render(&tuple).unwrap(), "(false, 7)");
}

#[test]
fn test_unsupported_values_return_typed_errors() {
    let structure = make::structure(&typ::bool(), Vec::new(), Span::default());
    assert_eq!(
        P4Unparser::new().render(&structure),
        Err(P4UnparseError::UnsupportedValue("Struct"))
    );
}

#[test]
fn test_print_hints_are_loaded_from_all_runtime_stages() {
    let al_spec = vec![p4spec_rust::phrase! {
        node: al::ast::DefKind::Typ(al::ast::TypDef {
            id: id("Wrapper"),
            tparams: Vec::new(),
            def_typ: hinted_def_type(),
            hints: Vec::new(),
        }),
        span: Span::default(),
    }];
    let sl_spec = vec![p4spec_rust::phrase! {
        node: sl::ast::DefKind::Typ(sl::ast::TypDef {
            id: id("Wrapper"),
            tparams: Vec::new(),
            def_typ: hinted_def_type(),
            hints: Vec::new(),
        }),
        span: Span::default(),
    }];
    let pl_spec = vec![pl::annot::Annotated::new(p4spec_rust::phrase! {
        node: pl::ast::DefKind::Typ(pl::ast::TypDef {
            id: id("Wrapper"),
            tparams: Vec::new(),
            def_typ: hinted_def_type(),
        }),
        span: Span::default(),
    })];
    let value = wrapped_text();

    assert_eq!(
        P4Unparser::from_al_spec(&al_spec).render(&value),
        Ok("show payload".to_owned())
    );
    assert_eq!(
        P4Unparser::from_sl_spec(&sl_spec).render(&value),
        Ok("show payload".to_owned())
    );
    assert_eq!(
        P4Unparser::from_pl_spec(&pl_spec).render(&value),
        Ok("show payload".to_owned())
    );
}
