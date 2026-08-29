use p4spec_rust::{
    lang::{
        common::{
            Id,
            notation::{atom::Atom, mixfix::Mixfix},
            noted::Noted,
            source::{Position, Span, Spanned},
        },
        il::ast,
        xl,
    },
    pass::algo::{
        self, AlgoErrorKind,
        binding::{
            bind::{self, Binding, Bindings},
            collect,
            context::Context,
            dimension,
            pattern::{self, PatternSet, PatternSets},
            shallow,
        },
    },
    runtime::{
        sta::Dim,
        types::{TypeDef, typ},
    },
};

fn span(line: i64) -> Span {
    let position = Position::new("algorithmic.watsup", line, 0);
    Span::new(position.clone(), position)
}

fn id(name: &str, line: i64) -> Id {
    Spanned::new(name.to_owned(), span(line))
}

fn exp(kind: ast::ExpKind, note: ast::TypKind, line: i64) -> ast::Exp {
    Spanned::new(Noted::new(kind, note), span(line))
}

fn var_exp(name: &str, line: i64) -> ast::Exp {
    exp(ast::ExpKind::Var(id(name, line)), ast::TypKind::Bool, line)
}

fn not_typ(name: &str, line: i64) -> ast::NotTyp {
    let atom = Spanned::new(Atom::Keyword(name.to_owned()), span(line));
    Spanned::new(Mixfix::Atom(atom), span(line))
}

fn pattern_set(names: &[&str]) -> PatternSet {
    names
        .iter()
        .enumerate()
        .map(|(index, name)| not_typ(name, index as i64 + 1))
        .collect()
}

#[test]
fn unsupported_conversion_uses_the_first_definition_span() {
    let spec = vec![
        Spanned::new(
            ast::DefKind::ExternTyp(ast::ExternTyp {
                id: id("first", 41),
                hints: vec![],
            }),
            span(41),
        ),
        Spanned::new(
            ast::DefKind::ExternTyp(ast::ExternTyp {
                id: id("last", 43),
                hints: vec![],
            }),
            span(43),
        ),
    ];
    let error = algo::convert(&spec).expect_err("foundation conversion stub");

    assert_eq!(error.kind, AlgoErrorKind::Unsupported);
    assert_eq!(error.span, span(41));
}

#[test]
fn context_loads_type_and_metavariable_definitions() {
    let extern_id = id("extern_type", 1);
    let defined_id = id("defined_type", 2);
    let variable_id = id("value", 3);
    let bool_typ = Spanned::new(ast::TypKind::Bool, span(2));
    let def_typ = Spanned::new(ast::DefTypKind::Plain(bool_typ.clone()), span(2));
    let spec = vec![
        Spanned::new(
            ast::DefKind::ExternTyp(ast::ExternTyp {
                id: extern_id.clone(),
                hints: vec![],
            }),
            span(1),
        ),
        Spanned::new(
            ast::DefKind::Typ(ast::TypDef {
                id: defined_id.clone(),
                tparams: vec![],
                def_typ: def_typ.clone(),
                hints: vec![],
            }),
            span(2),
        ),
        Spanned::new(
            ast::DefKind::Var(ast::VarDef {
                id: variable_id.clone(),
                typ: bool_typ.clone(),
                hints: vec![],
            }),
            span(3),
        ),
    ];

    let mut context = Context::new();
    context.load_spec(&spec);

    assert_eq!(context.tdenv.get(&extern_id), Some(&TypeDef::Extern));
    assert_eq!(
        context.tdenv.get(&defined_id),
        Some(&TypeDef::Defined(vec![], Box::new(def_typ)))
    );
    assert_eq!(context.menv.get(&variable_id), Some(&bool_typ));
    assert!(context.menv.contains_key(&id("bool", 99)));
}

#[test]
fn binding_union_keeps_the_first_span_and_marks_repetition() {
    let id_first = id("x", 1);
    let id_second = id("x", 2);
    let dim = Dim::new(typ::bool(), vec![]);
    let mut bindings_l = Bindings::new();
    bindings_l.insert(id_first.clone(), Binding::Single(dim.clone()));
    let mut bindings_r = Bindings::new();
    bindings_r.insert(id_second, Binding::Single(dim.clone()));

    let bindings = bind::union(bindings_l, bindings_r).expect("equivalent dimensions");

    assert_eq!(bindings.keys().next(), Some(&id_first));
    let Binding::Multiple(actual) = bindings.get(&id_first).expect("merged binding") else {
        panic!("expected a repeated binding");
    };
    assert!(actual.equiv(&dim));
}

#[test]
fn binding_union_rejects_conflicting_dimensions_at_the_first_key() {
    let id_first = id("x", 4);
    let id_second = id("x", 8);
    let mut bindings_l = Bindings::new();
    bindings_l.insert(
        id_first.clone(),
        Binding::Single(Dim::new(typ::bool(), vec![])),
    );
    let mut bindings_r = Bindings::new();
    bindings_r.insert(
        id_second,
        Binding::Single(Dim::new(typ::bool(), vec![ast::Iter::List])),
    );

    let error = bind::union(bindings_l, bindings_r).expect_err("conflicting dimensions");

    assert_eq!(error.kind, AlgoErrorKind::InconsistentDimensions);
    assert_eq!(error.span, id_first.span);
}

#[test]
fn dimension_inference_keeps_the_minimal_occurrence() {
    let iterated_var = var_exp("x", 2);
    let iterated = exp(
        ast::ExpKind::Iter(Box::new(iterated_var), (ast::Iter::List, vec![])),
        ast::TypKind::Iter(Box::new(typ::bool()), ast::Iter::List),
        3,
    );
    let direct = var_exp("x", 4);
    let tuple = exp(
        ast::ExpKind::Tuple(vec![iterated, direct]),
        ast::TypKind::Tuple(vec![]),
        1,
    );

    let dimensions = dimension::infer_exp(&tuple);
    let (stored_id, actual) = dimensions.iter().next().expect("inferred variable");

    assert_eq!(stored_id.span, span(2));
    assert!(actual.equiv(&Dim::new(typ::bool(), vec![])));
}

#[test]
fn collection_rejects_a_binding_inside_a_noninvertible_operator() {
    let variable = var_exp("x", 7);
    let negated = exp(
        ast::ExpKind::Un(
            ast::UnOp::Bool(xl::bool::UnOp::Not),
            ast::OpTyp::Bool,
            Box::new(variable),
        ),
        ast::TypKind::Bool,
        6,
    );

    let error =
        collect::collect_exp(&Context::new(), &negated).expect_err("binding under unary operator");

    assert_eq!(
        error.kind,
        AlgoErrorKind::NonInvertibleBinding("unary operator")
    );
    assert_eq!(error.span, span(7));
}

#[test]
fn expression_collection_reports_right_associated_conflict_span() {
    let iterated = exp(
        ast::ExpKind::Iter(Box::new(var_exp("x", 3)), (ast::Iter::List, vec![])),
        ast::TypKind::Iter(Box::new(typ::bool()), ast::Iter::List),
        3,
    );
    let tuple = exp(
        ast::ExpKind::Tuple(vec![var_exp("x", 1), var_exp("x", 2), iterated]),
        ast::TypKind::Tuple(vec![]),
        1,
    );

    let error = collect::collect_exp(&Context::new(), &tuple)
        .expect_err("third occurrence conflicts with the repeated tail binding");

    assert_eq!(error.kind, AlgoErrorKind::InconsistentDimensions);
    assert_eq!(error.span, span(2));
}

#[test]
fn argument_collection_reports_right_associated_conflict_span() {
    let iterated = exp(
        ast::ExpKind::Iter(Box::new(var_exp("x", 3)), (ast::Iter::List, vec![])),
        ast::TypKind::Iter(Box::new(typ::bool()), ast::Iter::List),
        3,
    );
    let args = [var_exp("x", 1), var_exp("x", 2), iterated]
        .into_iter()
        .map(|exp| Spanned::new(ast::ArgKind::Exp(Box::new(exp)), span(1)))
        .collect::<Vec<_>>();

    let error = collect::collect_args(&Context::new(), &args)
        .expect_err("third occurrence conflicts with the repeated tail binding");

    assert_eq!(error.kind, AlgoErrorKind::InconsistentDimensions);
    assert_eq!(error.span, span(2));
}

#[test]
fn shallow_cases_accept_only_iterated_variables_as_arguments() {
    let variable = var_exp("x", 1);
    let iterated = exp(
        ast::ExpKind::Iter(Box::new(variable), (ast::Iter::List, vec![])),
        ast::TypKind::Iter(Box::new(typ::bool()), ast::Iter::List),
        1,
    );
    let shallow_case = exp(
        ast::ExpKind::Case(Box::new(Mixfix::Arg(iterated))),
        ast::TypKind::Bool,
        1,
    );
    let nested_tuple = exp(
        ast::ExpKind::Tuple(vec![var_exp("x", 2)]),
        ast::TypKind::Tuple(vec![typ::bool()]),
        2,
    );
    let deep_case = exp(
        ast::ExpKind::Case(Box::new(Mixfix::Arg(nested_tuple))),
        ast::TypKind::Bool,
        2,
    );

    assert!(shallow::check_exp(&shallow_case));
    assert!(!shallow::check_exp(&deep_case));
}

#[test]
fn pattern_overlap_requires_intersection_in_every_dimension() {
    let owner_span = span(1);
    let pattern_a: PatternSets = vec![pattern_set(&["A", "B"]), pattern_set(&["X"])];
    let pattern_b: PatternSets = vec![pattern_set(&["B"]), pattern_set(&["X", "Y"])];
    let pattern_c: PatternSets = vec![pattern_set(&["B"]), pattern_set(&["Y"])];

    assert!(pattern::has_overlap(&owner_span, &pattern_a, &pattern_b).expect("matching arity"));
    assert!(!pattern::has_overlap(&owner_span, &pattern_a, &pattern_c).expect("matching arity"));
}

#[test]
fn pattern_arity_errors_use_the_owning_source_span() {
    let owner_span = span(31);
    let patterns_l: PatternSets = vec![pattern_set(&["A"])];
    let patterns_r: PatternSets = vec![pattern_set(&["A"]), pattern_set(&["B"])];

    let error = pattern::has_overlap(&owner_span, &patterns_l, &patterns_r)
        .expect_err("different pattern arities");

    assert_eq!(
        error.kind,
        AlgoErrorKind::PatternArityMismatch {
            expected: 1,
            actual: 2,
        }
    );
    assert_eq!(error.span, owner_span);
}

#[test]
fn pattern_sets_order_mixfix_structure_before_rendered_text() {
    let argument = Spanned::new(Mixfix::Arg(typ::bool()), span(2));
    let atom = not_typ("A", 1);
    let patterns: PatternSet = [atom, argument].into_iter().collect();
    let ordered = patterns.iter().collect::<Vec<_>>();

    assert!(matches!(ordered[0].node, Mixfix::Arg(_)));
    assert!(matches!(ordered[1].node, Mixfix::Atom(_)));
}

#[test]
fn pattern_subtraction_preserves_cartesian_fragment_order() {
    let owner_span = span(1);
    let total: PatternSets = vec![pattern_set(&["A", "B"]), pattern_set(&["X", "Y"])];
    let covered: PatternSets = vec![pattern_set(&["A"]), pattern_set(&["X"])];

    let missing = pattern::subtract(&owner_span, &total, &covered).expect("matching arity");

    assert_eq!(
        missing,
        vec![
            vec![pattern_set(&["B"]), pattern_set(&["X", "Y"])],
            vec![pattern_set(&["A"]), pattern_set(&["Y"])],
        ]
    );
}
