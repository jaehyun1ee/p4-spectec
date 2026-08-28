use p4spec_rust::{
    lang::{
        common::source::{Position, Span, Spanned},
        hints::input::InputHint,
        il::ast::{self, Iter, TypKind},
    },
    runtime::{
        sta::{Dim, FEnv, Func, IHEnv, MEnv, REnv, Rel, TDEnv, VEnv},
        types::TypeDef,
    },
};

fn span(file: &str) -> Span {
    Span::new(Position::new(file, 1, 0), Position::new(file, 1, 1))
}

fn id(name: &str, file: &str) -> ast::Id {
    Spanned::new(name.to_owned(), span(file))
}

fn typ(kind: TypKind, file: &str) -> ast::Typ {
    Spanned::new(kind, span(file))
}

#[test]
fn type_dimensions_ignore_spans_and_track_iterator_prefixes() {
    let bool_a = Dim::new(typ(TypKind::Bool, "a"), vec![Iter::Opt]);
    let bool_b = Dim::new(typ(TypKind::Bool, "b"), vec![Iter::Opt]);
    let bool_list = bool_a.clone().add_iter(Iter::List);

    assert!(bool_a.equiv(&bool_b));
    assert!(bool_a.sub(&bool_list));
    assert!(!bool_list.sub(&bool_a));
}

#[test]
fn static_environments_share_source_insensitive_identifier_identity() {
    let mut venv = VEnv::new();
    venv.insert(
        id("x", "definition"),
        Dim::new(typ(TypKind::Bool, "type"), vec![]),
    );
    assert!(venv.get(&id("x", "use")).is_some());

    let mut menv = MEnv::new();
    menv.insert(id("M", "definition"), typ(TypKind::Text, "type"));
    assert!(menv.get(&id("M", "use")).is_some());

    let mut tdenv = TDEnv::new();
    tdenv.insert(id("T", "definition"), TypeDef::Extern);
    assert!(tdenv.get(&id("T", "use")).is_some());

    let rel = Rel::Extern {
        not_typ: Box::new(Spanned::new(
            p4spec_rust::lang::common::notation::mixfix::Mixfix::Arg(typ(
                TypKind::Bool,
                "relation-type",
            )),
            span("relation"),
        )),
        input_hint: InputHint::new(vec![0]),
    };
    let mut renv = REnv::new();
    renv.insert(id("R", "definition"), rel);
    assert!(renv.get(&id("R", "use")).is_some());

    let mut ihenv = IHEnv::new();
    ihenv.insert(id("input", "definition"), InputHint::new(vec![1]));
    assert_eq!(ihenv.get(&id("input", "use")).unwrap().indices(), &[1]);

    let func = Func::Extern {
        tparams: vec![],
        params: vec![],
        typ_ret: Box::new(typ(TypKind::Bool, "result")),
    };
    let mut fenv = FEnv::new();
    fenv.insert(id("f", "definition"), func);
    assert!(fenv.get(&id("f", "use")).is_some());
}
