use super::*;

#[test]
fn test_syntax_equality_ignores_spans_and_subcheck_strategy() {
    let exp_l: il::ast::Exp = p4spec_rust::note_phrase! { node: il::ast::ExpKind::Sub(
    Box::new(variable("x")),
    Box::new(typ()),
    Box::new(il::ast::Subcheck::Skip),
    ), note: il::ast::TypKind::Bool, span: span("left") };
    let exp_r: il::ast::Exp = p4spec_rust::note_phrase! { node: il::ast::ExpKind::Sub(
    Box::new(variable("x")),
    Box::new(typ()),
    Box::new(il::ast::Subcheck::Recurse(typ())),
    ), note: il::ast::TypKind::Text, span: span("right") };

    assert!(exp_l.syntax_eq(&exp_r));
    assert!(
        std::slice::from_ref(&il::ast::Subcheck::Skip)
            .syntax_eq(&[il::ast::Subcheck::Recurse(typ()), il::ast::Subcheck::Skip,])
    );
    assert!(id("name").syntax_eq(&id("name")));
    assert!(arg_exp("x").syntax_eq(&arg_exp("x")));
    assert!(
        p4spec_rust::phrase! {
            node: al::ast::PremKind::If(al::ast::IfPrem { exp: variable("x") }),
            span: span("prem"),
        }
        .syntax_eq(&p4spec_rust::phrase! {
            node: al::ast::PremKind::If(al::ast::IfPrem { exp: variable("x") }),
            span: span("other-prem"),
        })
    );
}
#[test]
fn test_syntax_equality_distinguishes_recursive_operands_variants_and_collection_rules() {
    let value = |kind| {
        p4spec_rust::note_phrase! {
            node: kind,
            note: il::ast::TypKind::Bool,
            span: span("value"),
        }
    };
    let value_recursive = value(il::ast::ValueKind::List(vec![value(
        il::ast::ValueKind::Struct(vec![(atom(), value(il::ast::ValueKind::Bool(true)))]),
    )]));
    let value_recursive_changed = value(il::ast::ValueKind::List(vec![value(
        il::ast::ValueKind::Struct(vec![(atom(), value(il::ast::ValueKind::Bool(false)))]),
    )]));
    let exp_cases = [
        (variable("x"), variable("x"), true),
        (variable("x"), variable("y"), false),
        (
            expr(il::ast::ExpKind::Tuple(vec![variable("x")])),
            expr(il::ast::ExpKind::Tuple(vec![variable("x"), variable("y")])),
            false,
        ),
    ];
    for (exp_l, exp_r, is_equal) in exp_cases {
        assert_eq!(exp_l.syntax_eq(&exp_r), is_equal);
    }
    assert!(!value_recursive.syntax_eq(&value_recursive_changed));
    assert!(
        !value(il::ast::ValueKind::Bool(true))
            .syntax_eq(&value(il::ast::ValueKind::Text("true".to_owned())))
    );

    let path_root = || {
        p4spec_rust::note_phrase! {
            node: il::ast::PathKind::Root,
            note: il::ast::TypKind::Bool,
            span: span("root"),
        }
    };
    let path_x: il::ast::Path = p4spec_rust::note_phrase! {
        node: il::ast::PathKind::Idx(Box::new(path_root()), Box::new(variable("x"))),
        note: il::ast::TypKind::Bool,
        span: span("path-x"),
    };
    let path_y: il::ast::Path = p4spec_rust::note_phrase! {
        node: il::ast::PathKind::Idx(Box::new(path_root()), Box::new(variable("y"))),
        note: il::ast::TypKind::Bool,
        span: span("path-y"),
    };
    assert!(path_x.syntax_eq(&path_x));
    assert!(!path_x.syntax_eq(&path_y));
    assert!(
        !il::ast::Pattern::List(il::ast::ListPattern::Nil)
            .syntax_eq(&il::ast::Pattern::List(il::ast::ListPattern::Cons))
    );

    let prem_rule = |input_hint| {
        p4spec_rust::phrase! { node: al::ast::PremKind::Rule(al::ast::RulePrem {
            id: id("r"),
            not_exp: not_exp("x"),
            input_hint,
        }), span: span("rule") }
    };
    assert!(prem_rule(InputHint::new(vec![0])).syntax_eq(&prem_rule(InputHint::new(vec![0]))));
    assert!(!prem_rule(InputHint::new(vec![0])).syntax_eq(&prem_rule(InputHint::new(vec![1]))));
    assert!(
        !p4spec_rust::phrase! {
            node: al::ast::PremKind::If(al::ast::IfPrem { exp: variable("x") }),
            span: span("if"),
        }
        .syntax_eq(&p4spec_rust::phrase! {
            node: al::ast::PremKind::Debug(al::ast::DebugPrem { exp: variable("x") }),
            span: span("debug"),
        })
    );
    let prem_iter = |vars_bound, vars_bind| il::ast::PremIter {
        iter: il::ast::Iter::List,
        vars_bound,
        vars_bind,
    };
    let var_x = il::ast::Var {
        id: id("x"),
        typ: typ(),
        iters: Vec::new(),
    };
    let var_y = il::ast::Var {
        id: id("y"),
        typ: typ(),
        iters: Vec::new(),
    };
    assert!(
        prem_iter(vec![var_x.clone(), var_y.clone()], vec![var_x.clone()]).syntax_eq(&prem_iter(
            vec![var_y.clone(), var_x.clone()],
            vec![var_x.clone()]
        ))
    );
    assert!(
        !prem_iter(vec![var_x.clone()], vec![var_x.clone()])
            .syntax_eq(&prem_iter(vec![var_y.clone()], vec![var_x.clone()]))
    );
    assert!(
        !prem_iter(vec![var_x.clone()], vec![var_x.clone()])
            .syntax_eq(&prem_iter(vec![var_x.clone()], vec![var_y.clone()]))
    );
    assert!(![variable("x"), variable("y")].syntax_eq(&[variable("y"), variable("x")]));
    assert!([var_x.clone(), var_y.clone()].syntax_eq(&[var_y, var_x]));
    assert!(!std::slice::from_ref(&value_recursive).syntax_eq(&[value_recursive_changed]));
    assert!(![arg_exp("x")].syntax_eq(&[arg_exp("y")]));
}
