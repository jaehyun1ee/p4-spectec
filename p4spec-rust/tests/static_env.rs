use std::cmp::Ordering;

use p4spec_rust::{
    lang::{
        common::source::{Position, Span, Spanned},
        hints::input::InputHint,
        il::ast::{self, Iter, TypKind},
    },
    runtime::{
        static_env::{
            Function, FunctionEnvironment, InputHintEnvironment, MetavariableEnvironment, Relation,
            RelationEnvironment, TDEnv, TypeDimension, VariableEnvironment,
        },
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
fn type_dimensions_compare_without_spans_and_track_iterator_prefixes() {
    let bool_a = TypeDimension::new(typ(TypKind::Bool, "a"), vec![Iter::Opt]);
    let bool_b = TypeDimension::new(typ(TypKind::Bool, "b"), vec![Iter::Opt]);
    let bool_list = bool_a.clone().with_iter(Iter::List);
    let text = TypeDimension::new(typ(TypKind::Text, "text"), vec![]);

    assert_eq!(bool_a.compare(&bool_b), Ordering::Equal);
    assert!(bool_a.equivalent(&bool_b));
    assert!(bool_a.is_subdimension_of(&bool_list));
    assert!(!bool_list.is_subdimension_of(&bool_a));
    assert_eq!(bool_a.compare(&text), Ordering::Less);
    assert_eq!(bool_list.iters(), &[Iter::Opt, Iter::List]);
}

#[test]
fn static_environments_share_source_insensitive_identifier_identity() {
    let mut variables = VariableEnvironment::new();
    variables.insert(
        id("x", "definition"),
        TypeDimension::new(typ(TypKind::Bool, "type"), vec![]),
    );
    assert!(variables.get(&id("x", "use")).is_some());

    let mut metavariables = MetavariableEnvironment::new();
    metavariables.insert(id("M", "definition"), typ(TypKind::Text, "type"));
    assert!(metavariables.get(&id("M", "use")).is_some());

    let mut types = TDEnv::new();
    types.insert(id("T", "definition"), TypeDef::Extern);
    assert!(types.get(&id("T", "use")).is_some());

    let relation = Relation::Extern {
        notation_type: Box::new(Spanned::new(
            p4spec_rust::lang::common::notation::mixfix::Mixfix::Arg(typ(
                TypKind::Bool,
                "relation-type",
            )),
            span("relation"),
        )),
        input_hint: InputHint::new(vec![0]),
    };
    let mut relations = RelationEnvironment::new();
    relations.insert(id("R", "definition"), relation);
    assert!(relations.get(&id("R", "use")).is_some());

    let mut inputs = InputHintEnvironment::new();
    inputs.insert(id("input", "definition"), InputHint::new(vec![1]));
    assert_eq!(inputs.get(&id("input", "use")).unwrap().indices(), &[1]);

    let function = Function::Extern {
        type_parameters: vec![],
        parameters: vec![],
        result_type: Box::new(typ(TypKind::Bool, "result")),
    };
    let mut functions = FunctionEnvironment::new();
    functions.insert(id("f", "definition"), function);
    assert!(functions.get(&id("f", "use")).is_some());
}
