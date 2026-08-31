use p4spec_rust::{
    frontend::parse::parse_string,
    lang::{
        common::{
            Id,
            notation::{atom::Atom, mixfix::Mixfix},
            source::{NotePhrase, Position, Span},
        },
        hints::input::InputHint,
        il::ast,
        traits::eq::SyntaxEq,
        xl,
    },
    pass::{
        algo::{
            self, AlgoErrorKind,
            binding::{
                antiunify,
                bind::{self, Binding, Bindings},
                collect,
                context::Context,
                dimension,
                iteration::IterationContext,
                multiple, partial,
                pattern::{self, PatternSet, PatternSets},
                shallow,
            },
        },
        elaborate,
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
    p4spec_rust::phrase! { node: name.to_owned(), span:  span(line) }
}

fn exp(kind: ast::ExpKind, note: ast::TypKind, line: i64) -> ast::Exp {
    p4spec_rust::note_phrase! { node: kind, note:  note, span:  span(line) }
}

fn var_exp(name: &str, line: i64) -> ast::Exp {
    exp(ast::ExpKind::Var(id(name, line)), ast::TypKind::Bool, line)
}

fn typed_var_exp(name: &str, typ: &ast::Typ, line: i64) -> ast::Exp {
    exp(ast::ExpKind::Var(id(name, line)), typ.node.clone(), line)
}

fn iterated_var_exp(name: &str, typ: &ast::Typ, iter: ast::Iter, line: i64) -> ast::Exp {
    let exp_inner = typed_var_exp(name, typ, line);
    exp(
        ast::ExpKind::Iter(Box::new(exp_inner), (iter, vec![])),
        ast::TypKind::Iter(Box::new(typ.clone()), iter),
        line,
    )
}

fn exp_arg(exp: ast::Exp) -> ast::Arg {
    let span = exp.span.clone();
    p4spec_rust::phrase! { node: ast::ArgKind::Exp(Box::new(exp)), span:  span }
}

fn if_prem(exp: ast::Exp) -> ast::Prem {
    let span = exp.span.clone();
    p4spec_rust::phrase! { node: ast::PremKind::If(ast::IfPrem { exp }), span:  span }
}

fn function_spec(
    params: Vec<ast::Typ>,
    args: Vec<ast::Exp>,
    expression: ast::Exp,
    premises: Vec<ast::Prem>,
) -> ast::Spec {
    let typ =
        p4spec_rust::phrase! { node: expression.note.clone(), span:  expression.span.clone() };
    let clause = p4spec_rust::phrase! { node:
    ast::ClauseKind {
        args: args.into_iter().map(exp_arg).collect(),
        expression,
        premises,
    }, span:
    span(1) };
    let params = params
        .into_iter()
        .map(|typ| p4spec_rust::phrase! { node: ast::ParamKind::Exp(typ), span:  span(1) })
        .collect();
    vec![p4spec_rust::phrase! { node:
    ast::DefKind::FuncDec(ast::FuncDec {
        id: id("function", 1),
        tparams: vec![],
        params,
        typ,
        clauses: vec![clause],
        else_clause: None,
        hints: vec![],
    }), span:
    span(1) }]
}

fn joint_iteration(names: &[(&str, i64)], iter: ast::Iter, line: i64) -> ast::Exp {
    let typ_bool = typ::bool();
    let exps = names
        .iter()
        .map(|(name, line)| typed_var_exp(name, &typ_bool, *line))
        .collect::<Vec<_>>();
    let vars = names
        .iter()
        .map(|(name, line)| ast::Var {
            id: id(name, *line),
            typ: typ_bool.clone(),
            iters: vec![],
        })
        .collect::<Vec<_>>();
    let typ_tuple = p4spec_rust::phrase! { node: ast::TypKind::Tuple(vec![typ_bool; names.len()]), span:  span(line) };
    let exp_inner = exp(ast::ExpKind::Tuple(exps), typ_tuple.node.clone(), line);
    exp(
        ast::ExpKind::Iter(Box::new(exp_inner), (iter, vars)),
        ast::TypKind::Iter(Box::new(typ_tuple), iter),
        line,
    )
}

fn dimension_exp(name: &str, iter: ast::Iter, line: i64) -> ast::Exp {
    p4spec_rust::lang::il::var::as_exp(
        true,
        &ast::Var {
            id: id(name, line),
            typ: typ::bool(),
            iters: vec![iter],
        },
    )
}

fn len_exp(name: &str, line: i64) -> ast::Exp {
    let exp_inner = dimension_exp(name, ast::Iter::List, line);
    exp(
        ast::ExpKind::Len(Box::new(exp_inner)),
        ast::TypKind::Num(xl::num::Typ::Nat),
        line,
    )
}

fn equality_prem(exp_l: ast::Exp, exp_r: ast::Exp, line: i64) -> ast::Prem {
    let condition = exp(
        ast::ExpKind::Cmp(
            ast::CmpOp::Bool(xl::bool::CmpOp::Eq),
            ast::OpTyp::Bool,
            Box::new(exp_l),
            Box::new(exp_r),
        ),
        ast::TypKind::Bool,
        line,
    );
    if_prem(condition)
}

fn indexed_exp(base: ast::Exp, index: ast::Exp, note: ast::TypKind, line: i64) -> ast::Exp {
    exp(
        ast::ExpKind::Idx(Box::new(base), Box::new(index)),
        note,
        line,
    )
}

fn literal_index_exp(value: bool, line: i64) -> ast::Exp {
    let typ_bool = typ::bool();
    let base = exp(
        ast::ExpKind::List(vec![exp(
            ast::ExpKind::Bool(value),
            ast::TypKind::Bool,
            line,
        )]),
        typ::list(typ_bool).node,
        line,
    );
    let index = exp(
        ast::ExpKind::Num(ast::Num::Nat(0_u64.into())),
        ast::TypKind::Num(xl::num::Typ::Nat),
        line,
    );
    indexed_exp(base, index, ast::TypKind::Bool, line)
}

fn assert_index_guard_span(prem: &ast::Prem, expected_span: Span) {
    assert_eq!(prem.span, expected_span);
    let ast::PremKind::If(if_prem) = &prem.node else {
        panic!("expected index guard premise");
    };
    assert_eq!(if_prem.exp.span, expected_span);
    assert!(matches!(
        if_prem.exp.node,
        ast::ExpKind::Cmp(ast::CmpOp::Num(xl::num::CmpOp::Lt), _, _, _)
    ));
}

fn iteration_var(name: &str, typ: ast::Typ, line: i64) -> ast::Var {
    ast::Var {
        id: id(name, line),
        typ,
        iters: vec![],
    }
}

fn function_clause(spec: &p4spec_rust::lang::al::ast::Spec) -> &ast::Clause {
    let p4spec_rust::lang::al::ast::DefKind::FuncDec(function) = &spec[0].node else {
        panic!("expected function definition");
    };
    &function.clauses[0]
}

fn not_typ(name: &str, line: i64) -> ast::NotTyp {
    let atom = p4spec_rust::phrase! { node: Atom::Keyword(name.to_owned()), span:  span(line) };
    p4spec_rust::phrase! { node: Mixfix::Atom(atom), span:  span(line) }
}

fn pattern_set(names: &[&str]) -> PatternSet {
    names
        .iter()
        .enumerate()
        .map(|(index, name)| not_typ(name, index as i64 + 1))
        .collect()
}

#[test]
fn conversion_propagates_located_binding_errors() {
    let variable = var_exp("x", 41);
    let negated = exp(
        ast::ExpKind::Un(
            ast::UnOp::Bool(xl::bool::UnOp::Not),
            ast::OpTyp::Bool,
            Box::new(variable),
        ),
        ast::TypKind::Bool,
        40,
    );
    let spec = function_spec(
        vec![typ::bool()],
        vec![negated],
        exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, 42),
        vec![],
    );

    let error = algo::convert(&spec).expect_err("binding below a unary operator");

    assert_eq!(
        error.kind,
        AlgoErrorKind::NonInvertibleBinding("unary operator")
    );
    assert_eq!(error.span, span(41));
}

#[test]
fn conversion_inserts_index_guards_at_evaluation_sites_in_source_order() {
    fn assert_index_guard(
        prem: &ast::Prem,
        guard_span: Span,
        base_span: Span,
        index_name: &str,
        index_span: Span,
    ) {
        assert_eq!(prem.span, guard_span);
        let ast::PremKind::If(if_prem) = &prem.node else {
            panic!("expected index guard premise");
        };
        assert_eq!(if_prem.exp.span, guard_span);
        let ast::ExpKind::Cmp(
            ast::CmpOp::Num(xl::num::CmpOp::Lt),
            ast::OpTyp::Bool,
            exp_i,
            exp_len,
        ) = &if_prem.exp.node
        else {
            panic!("expected strict index bound");
        };
        assert_eq!(exp_i.span, index_span);
        assert!(matches!(&exp_i.node, ast::ExpKind::Var(id) if id.node == index_name));
        assert_eq!(exp_len.span, guard_span);
        let ast::ExpKind::Len(exp_base) = &exp_len.node else {
            panic!("expected indexed-base length");
        };
        assert_eq!(exp_base.span, base_span);
    }

    let typ_bool = typ::bool();
    let typ_nat = typ::nat();
    let typ_list = typ::list(typ_bool.clone());
    let exp_index_prem = exp(
        ast::ExpKind::Idx(
            Box::new(iterated_var_exp("xs", &typ_bool, ast::Iter::List, 10)),
            Box::new(typed_var_exp("i", &typ_nat, 11)),
        ),
        ast::TypKind::Bool,
        12,
    );
    let exp_condition = exp(
        ast::ExpKind::Cmp(
            ast::CmpOp::Bool(xl::bool::CmpOp::Eq),
            ast::OpTyp::Bool,
            Box::new(exp_index_prem),
            Box::new(exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, 13)),
        ),
        ast::TypKind::Bool,
        13,
    );
    let prem_source = if_prem(exp_condition);
    let exp_output = exp(
        ast::ExpKind::Idx(
            Box::new(iterated_var_exp("xs", &typ_bool, ast::Iter::List, 20)),
            Box::new(typed_var_exp("j", &typ_nat, 21)),
        ),
        ast::TypKind::Bool,
        22,
    );
    let spec = function_spec(
        vec![typ_list, typ_nat.clone(), typ_nat.clone()],
        vec![
            iterated_var_exp("xs", &typ_bool, ast::Iter::List, 2),
            typed_var_exp("i", &typ_nat, 3),
            typed_var_exp("j", &typ_nat, 4),
        ],
        exp_output.clone(),
        vec![prem_source.clone()],
    );

    let converted = algo::convert(&spec).expect("guarded conversion");
    let clause = function_clause(&converted);
    let [guard_premise, source_premise, guard_output] = clause.node.premises.as_slice() else {
        panic!("expected premise and output index guards");
    };

    assert_index_guard(guard_premise, span(12), span(10), "i", span(11));
    assert_eq!(source_premise, &prem_source);
    assert_index_guard(guard_output, span(22), span(20), "j", span(21));
    assert_eq!(clause.node.expression, exp_output);
    assert_eq!(clause.span, span(1));
}

#[test]
fn conversion_inserts_list_and_optional_iteration_guards_in_source_order() {
    fn dimension_name(exp: &ast::Exp, iter: ast::Iter) -> &str {
        let ast::ExpKind::Iter(exp_inner, (actual_iter, _)) = &exp.node else {
            panic!("expected dimension expression");
        };
        assert_eq!(*actual_iter, iter);
        let ast::ExpKind::Var(id) = &exp_inner.node else {
            panic!("expected dimension variable");
        };
        &id.node
    }

    fn list_pair(exp: &ast::Exp) -> (&str, &str) {
        let ast::ExpKind::Cmp(
            ast::CmpOp::Bool(xl::bool::CmpOp::Eq),
            ast::OpTyp::Bool,
            exp_l,
            exp_r,
        ) = &exp.node
        else {
            panic!("expected list-length equality");
        };
        let ast::ExpKind::Len(exp_l) = &exp_l.node else {
            panic!("expected left length");
        };
        let ast::ExpKind::Len(exp_r) = &exp_r.node else {
            panic!("expected right length");
        };
        (
            dimension_name(exp_l, ast::Iter::List),
            dimension_name(exp_r, ast::Iter::List),
        )
    }

    fn optional_name(exp: &ast::Exp) -> &str {
        let ast::ExpKind::Cmp(
            ast::CmpOp::Bool(xl::bool::CmpOp::Eq),
            ast::OpTyp::Bool,
            exp_l,
            exp_r,
        ) = &exp.node
        else {
            panic!("expected optional-presence equality");
        };
        assert!(matches!(exp_r.node, ast::ExpKind::Opt(None)));
        dimension_name(exp_l, ast::Iter::Opt)
    }

    fn optional_pair(exp: &ast::Exp) -> (&str, &str) {
        let ast::ExpKind::Bin(
            ast::BinOp::Bool(xl::bool::BinOp::Equiv),
            ast::OpTyp::Bool,
            exp_l,
            exp_r,
        ) = &exp.node
        else {
            panic!("expected optional-presence equivalence");
        };
        (optional_name(exp_l), optional_name(exp_r))
    }

    let typ_bool = typ::bool();
    let list_names = [("x", 2), ("y", 3), ("z", 4)];
    let optional_names = [("p", 5), ("q", 6), ("r", 7)];
    let mut params = vec![typ::list(typ_bool.clone()); list_names.len()];
    params.extend(vec![typ::opt(typ_bool.clone()); optional_names.len()]);
    let mut args = list_names
        .iter()
        .map(|(name, line)| iterated_var_exp(name, &typ_bool, ast::Iter::List, *line))
        .collect::<Vec<_>>();
    args.extend(
        optional_names
            .iter()
            .map(|(name, line)| iterated_var_exp(name, &typ_bool, ast::Iter::Opt, *line)),
    );
    let exp_list = joint_iteration(&list_names, ast::Iter::List, 20);
    let exp_optional = joint_iteration(&optional_names, ast::Iter::Opt, 21);
    let typ_output = ast::TypKind::Tuple(vec![
        p4spec_rust::phrase! { node: exp_list.note.clone(), span:  exp_list.span.clone() },
        p4spec_rust::phrase! { node: exp_optional.note.clone(), span:  exp_optional.span.clone() },
    ]);
    let exp_output = exp(
        ast::ExpKind::Tuple(vec![exp_list, exp_optional]),
        typ_output,
        22,
    );
    let spec = function_spec(params, args, exp_output, vec![]);

    let converted = algo::convert(&spec).expect("guarded joint iterations");
    let clause = function_clause(&converted);
    let [prem_list, prem_optional] = clause.node.premises.as_slice() else {
        panic!("expected list and optional guards");
    };

    assert_eq!(prem_list.span, Span::over(&[span(2), span(3), span(4)]));
    let ast::PremKind::If(if_list) = &prem_list.node else {
        panic!("expected list guard premise");
    };
    let ast::ExpKind::Bin(
        ast::BinOp::Bool(xl::bool::BinOp::And),
        ast::OpTyp::Bool,
        pair_xy,
        pair_yz,
    ) = &if_list.exp.node
    else {
        panic!("expected pairwise list guard conjunction");
    };
    assert_eq!(list_pair(pair_xy), ("x", "y"));
    assert_eq!(list_pair(pair_yz), ("y", "z"));

    assert_eq!(prem_optional.span, Span::over(&[span(5), span(6), span(7)]));
    let ast::PremKind::If(if_optional) = &prem_optional.node else {
        panic!("expected optional guard premise");
    };
    let ast::ExpKind::Bin(
        ast::BinOp::Bool(xl::bool::BinOp::And),
        ast::OpTyp::Bool,
        pair_pq,
        pair_qr,
    ) = &if_optional.exp.node
    else {
        panic!("expected pairwise optional guard conjunction");
    };
    assert_eq!(optional_pair(pair_pq), ("p", "q"));
    assert_eq!(optional_pair(pair_qr), ("q", "r"));
}

#[test]
fn conversion_omits_iteration_guards_entailed_by_prior_premises() {
    let typ_bool = typ::bool();
    let names = [("x", 2), ("y", 3), ("z", 4)];
    let args = names
        .iter()
        .map(|(name, line)| iterated_var_exp(name, &typ_bool, ast::Iter::List, *line))
        .collect::<Vec<_>>();
    let prem_xy = equality_prem(len_exp("x", 10), len_exp("y", 11), 12);
    let prem_yz = equality_prem(len_exp("y", 13), len_exp("z", 14), 15);
    let exp_output = joint_iteration(&[("x", 20), ("z", 21)], ast::Iter::List, 22);
    let spec = function_spec(
        vec![typ::list(typ_bool); names.len()],
        args,
        exp_output,
        vec![prem_xy.clone(), prem_yz.clone()],
    );

    let converted = algo::convert(&spec).expect("transitively guarded iteration");
    let premises = &function_clause(&converted).node.premises;

    assert_eq!(premises, &[prem_xy, prem_yz]);
}

#[test]
fn conversion_preserves_binding_match_and_cast_guards_before_bindings() {
    let parent_id = id("Parent", 1);
    let child_id = id("Child", 2);
    let parent_typ =
        p4spec_rust::phrase! { node: ast::TypKind::Var(parent_id.clone(), vec![]), span:  span(1) };
    let child_typ =
        p4spec_rust::phrase! { node: ast::TypKind::Var(child_id.clone(), vec![]), span:  span(2) };
    let parent_origin = p4spec_rust::phrase! { node: (parent_id.clone(), vec![]), span:  span(1) };
    let child_origin = p4spec_rust::phrase! { node: (child_id.clone(), vec![]), span:  span(2) };
    let parent_def = p4spec_rust::phrase! { node:
    ast::DefKind::Typ(ast::TypDef {
        id: parent_id,
        tparams: vec![],
        def_typ: p4spec_rust::phrase! { node:
            ast::DefTypKind::Variant(vec![
                (not_typ("A", 1), parent_origin.clone(), vec![]),
                (not_typ("B", 1), parent_origin, vec![]),
            ]), span:
            span(1) },
        hints: vec![],
    }), span:
    span(1) };
    let child_def = p4spec_rust::phrase! { node:
    ast::DefKind::Typ(ast::TypDef {
        id: child_id,
        tparams: vec![],
        def_typ: p4spec_rust::phrase! { node:
            ast::DefTypKind::Variant(vec![(not_typ("A", 2), child_origin, vec![])]), span:
            span(2) },
        hints: vec![],
    }), span:
    span(2) };
    let typ_bool = typ::bool();
    let typ_list = typ::list(typ_bool.clone());
    let exp_list = exp(
        ast::ExpKind::List(vec![typed_var_exp("item", &typ_bool, 10)]),
        typ_list.node.clone(),
        10,
    );
    let exp_upcast = exp(
        ast::ExpKind::UpCast(
            parent_typ.clone(),
            Box::new(typed_var_exp("child", &child_typ, 11)),
        ),
        parent_typ.node.clone(),
        11,
    );
    let exp_output = exp(
        ast::ExpKind::Tuple(vec![
            typed_var_exp("item", &typ_bool, 12),
            typed_var_exp("child", &child_typ, 12),
        ]),
        ast::TypKind::Tuple(vec![typ_bool.clone(), child_typ.clone()]),
        12,
    );
    let mut spec = function_spec(
        vec![typ_list, parent_typ],
        vec![exp_list, exp_upcast],
        exp_output,
        vec![],
    );
    spec.insert(0, child_def);
    spec.insert(0, parent_def);

    let converted = algo::convert(&spec).expect("partial binding conversion");
    let p4spec_rust::lang::al::ast::DefKind::FuncDec(function) = &converted[2].node else {
        panic!("expected converted function");
    };
    let [match_guard, list_binding, subtype_guard, cast_binding] =
        function.clauses[0].node.premises.as_slice()
    else {
        panic!("expected match/bind and subtype/downcast pairs");
    };

    assert!(matches!(
        &match_guard.node,
        ast::PremKind::If(ast::IfPrem {
            exp: NotePhrase {
                node: ast::ExpKind::Match(_, ast::Pattern::List(ast::ListPattern::Fixed(1))),
                ..
            }
        })
    ));
    assert!(matches!(&list_binding.node, ast::PremKind::Let(_)));
    assert!(matches!(
        &subtype_guard.node,
        ast::PremKind::If(ast::IfPrem {
            exp: NotePhrase {
                node: ast::ExpKind::Sub(_, typ, _),
                ..
            }
        }) if typ.syntax_eq(&child_typ)
    ));
    assert!(matches!(
        &cast_binding.node,
        ast::PremKind::Let(ast::LetPrem {
            exp_r: NotePhrase {
                node: ast::ExpKind::DownCast(typ, _),
                ..
            },
            ..
        }) if typ.syntax_eq(&child_typ)
    ));
}

#[test]
fn conversion_preserves_numeric_and_slice_checks_before_output_guards() {
    let typ_nat = typ::nat();
    let typ_list = typ::list(typ_nat.clone());
    let exp_zero = exp(
        ast::ExpKind::Num(ast::Num::Nat(0_u64.into())),
        ast::TypKind::Num(xl::num::Typ::Nat),
        10,
    );
    let exp_nonzero = exp(
        ast::ExpKind::Cmp(
            ast::CmpOp::Bool(xl::bool::CmpOp::Ne),
            ast::OpTyp::Bool,
            Box::new(typed_var_exp("d", &typ_nat, 10)),
            Box::new(exp_zero),
        ),
        ast::TypKind::Bool,
        10,
    );
    let prem_nonzero = if_prem(exp_nonzero);
    let exp_end = exp(
        ast::ExpKind::Bin(
            ast::BinOp::Num(xl::num::BinOp::Add),
            ast::OpTyp::Nat,
            Box::new(typed_var_exp("offset", &typ_nat, 11)),
            Box::new(typed_var_exp("length", &typ_nat, 11)),
        ),
        ast::TypKind::Num(xl::num::Typ::Nat),
        11,
    );
    let exp_base_for_len = iterated_var_exp("xs", &typ_nat, ast::Iter::List, 12);
    let exp_length = exp(
        ast::ExpKind::Len(Box::new(exp_base_for_len)),
        ast::TypKind::Num(xl::num::Typ::Nat),
        12,
    );
    let exp_slice_bound = exp(
        ast::ExpKind::Cmp(
            ast::CmpOp::Num(xl::num::CmpOp::Le),
            ast::OpTyp::Bool,
            Box::new(exp_end),
            Box::new(exp_length),
        ),
        ast::TypKind::Bool,
        12,
    );
    let prem_slice_bound = if_prem(exp_slice_bound);
    let exp_index = exp(
        ast::ExpKind::Idx(
            Box::new(iterated_var_exp("xs", &typ_nat, ast::Iter::List, 20)),
            Box::new(typed_var_exp("index", &typ_nat, 21)),
        ),
        ast::TypKind::Num(xl::num::Typ::Nat),
        22,
    );
    let exp_division = exp(
        ast::ExpKind::Bin(
            ast::BinOp::Num(xl::num::BinOp::Div),
            ast::OpTyp::Nat,
            Box::new(exp_index),
            Box::new(typed_var_exp("d", &typ_nat, 23)),
        ),
        ast::TypKind::Num(xl::num::Typ::Nat),
        23,
    );
    let exp_slice = exp(
        ast::ExpKind::Slice(
            Box::new(iterated_var_exp("xs", &typ_nat, ast::Iter::List, 24)),
            Box::new(typed_var_exp("offset", &typ_nat, 24)),
            Box::new(typed_var_exp("length", &typ_nat, 24)),
        ),
        typ_list.node.clone(),
        24,
    );
    let exp_remainder = exp(
        ast::ExpKind::Bin(
            ast::BinOp::Num(xl::num::BinOp::Mod),
            ast::OpTyp::Nat,
            Box::new(typed_var_exp("value", &typ_nat, 25)),
            Box::new(typed_var_exp("d", &typ_nat, 25)),
        ),
        ast::TypKind::Num(xl::num::Typ::Nat),
        25,
    );
    let exp_output = exp(
        ast::ExpKind::Tuple(vec![exp_division, exp_slice, exp_remainder]),
        ast::TypKind::Tuple(vec![typ_nat.clone(), typ_list.clone(), typ_nat.clone()]),
        26,
    );
    let params = vec![
        typ_list,
        typ_nat.clone(),
        typ_nat.clone(),
        typ_nat.clone(),
        typ_nat.clone(),
        typ_nat.clone(),
    ];
    let args = vec![
        iterated_var_exp("xs", &typ_nat, ast::Iter::List, 2),
        typed_var_exp("index", &typ_nat, 3),
        typed_var_exp("offset", &typ_nat, 4),
        typed_var_exp("length", &typ_nat, 5),
        typed_var_exp("value", &typ_nat, 6),
        typed_var_exp("d", &typ_nat, 7),
    ];
    let spec = function_spec(
        params,
        args,
        exp_output,
        vec![prem_nonzero.clone(), prem_slice_bound.clone()],
    );

    let converted = algo::convert(&spec).expect("numeric and slice conversion");
    let premises = &function_clause(&converted).node.premises;
    let [actual_nonzero, actual_slice_bound, index_guard] = premises.as_slice() else {
        panic!("expected two explicit checks and one index guard");
    };

    assert_eq!(actual_nonzero, &prem_nonzero);
    assert_eq!(actual_slice_bound, &prem_slice_bound);
    let ast::PremKind::If(if_index) = &index_guard.node else {
        panic!("expected index guard");
    };
    assert!(matches!(
        if_index.exp.node,
        ast::ExpKind::Cmp(ast::CmpOp::Num(xl::num::CmpOp::Lt), _, _, _)
    ));
    assert_eq!(index_guard.span, span(22));
}

#[test]
fn conversion_distinguishes_let_must_guards_from_insert_guards() {
    let typ_bool = typ::bool();
    let exp_l = joint_iteration(&[("bound_l", 20), ("bound_r", 21)], ast::Iter::List, 22);
    let exp_r = joint_iteration(&[("input_l", 23), ("input_r", 24)], ast::Iter::List, 25);
    let prem_equality = equality_prem(exp_l, exp_r, 30);
    let exp_output = joint_iteration(&[("bound_l", 20), ("bound_r", 21)], ast::Iter::List, 40);
    let spec = function_spec(
        vec![typ::list(typ_bool.clone()), typ::list(typ_bool.clone())],
        vec![
            iterated_var_exp("input_l", &typ_bool, ast::Iter::List, 2),
            iterated_var_exp("input_r", &typ_bool, ast::Iter::List, 3),
        ],
        exp_output.clone(),
        vec![prem_equality],
    );

    let converted = algo::convert(&spec).expect("let guard conversion");
    let clause = function_clause(&converted);
    let [right_guard, let_premise] = clause.node.premises.as_slice() else {
        panic!("expected only the right guard before the generated let premise");
    };

    assert_eq!(right_guard.span, Span::over(&[span(23), span(24)]));
    assert!(matches!(right_guard.node, ast::PremKind::If(_)));
    assert_eq!(let_premise.span, span(30));
    let ast::PremKind::Let(let_prem) = &let_premise.node else {
        panic!("expected binding analysis to produce a let premise");
    };
    assert!(matches!(let_prem.exp_l.node, ast::ExpKind::Iter(_, _)));
    assert!(matches!(let_prem.exp_r.node, ast::ExpKind::Iter(_, _)));
    assert_eq!(clause.node.expression, exp_output);
}

#[test]
fn conversion_distinguishes_iterated_must_guards_from_insert_guards() {
    let typ_bool = typ::bool();
    let exp_condition = exp(
        ast::ExpKind::Cmp(
            ast::CmpOp::Bool(xl::bool::CmpOp::Eq),
            ast::OpTyp::Bool,
            Box::new(typed_var_exp("left", &typ_bool, 12)),
            Box::new(typed_var_exp("right", &typ_bool, 13)),
        ),
        ast::TypKind::Bool,
        14,
    );
    let prem_iterated = p4spec_rust::phrase! { node:
    ast::PremKind::Iter(ast::IteratedPrem {
        prem: Box::new(if_prem(exp_condition)),
        iter_prem: ast::IterPrem {
            iter: ast::Iter::List,
            vars_bound: vec![
                iteration_var("left", typ_bool.clone(), 10),
                iteration_var("right", typ_bool.clone(), 11),
            ],
            vars_bind: vec![],
        },
    }), span:
    span(15) };
    let insert_spec = function_spec(
        vec![typ::list(typ_bool.clone()), typ::list(typ_bool.clone())],
        vec![
            iterated_var_exp("left", &typ_bool, ast::Iter::List, 2),
            iterated_var_exp("right", &typ_bool, ast::Iter::List, 3),
        ],
        exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, 16),
        vec![prem_iterated],
    );

    let converted = algo::convert(&insert_spec).expect("iterated insertion conversion");
    let [joint_guard, source_premise] = function_clause(&converted).node.premises.as_slice() else {
        panic!("expected a joint guard before the iterated premise");
    };
    assert_eq!(joint_guard.span, Span::over(&[span(10), span(11)]));
    assert!(matches!(joint_guard.node, ast::PremKind::If(_)));
    assert_eq!(source_premise.span, span(14));
    assert!(matches!(source_premise.node, ast::PremKind::Iter(_)));

    let prem_binding = p4spec_rust::phrase! { node:
    ast::PremKind::Iter(ast::IteratedPrem {
        prem: Box::new(equality_prem(
            typed_var_exp("output", &typ_bool, 23),
            typed_var_exp("input", &typ_bool, 22),
            24,
        )),
        iter_prem: ast::IterPrem {
            iter: ast::Iter::List,
            vars_bound: vec![iteration_var("input", typ_bool.clone(), 22)],
            vars_bind: vec![],
        },
    }), span:
    span(25) };
    let exp_output = joint_iteration(&[("input", 22), ("output", 23)], ast::Iter::List, 30);
    let must_spec = function_spec(
        vec![typ::list(typ_bool.clone())],
        vec![iterated_var_exp("input", &typ_bool, ast::Iter::List, 20)],
        exp_output.clone(),
        vec![prem_binding],
    );

    let converted = algo::convert(&must_spec).expect("iterated binding conversion");
    let clause = function_clause(&converted);
    let [source_premise] = clause.node.premises.as_slice() else {
        panic!("bound-plus-bind guard must suppress the matching output guard");
    };
    assert_eq!(source_premise.span, span(24));
    let ast::PremKind::Iter(iterated) = &source_premise.node else {
        panic!("expected analyzed iterated binding premise");
    };
    assert_eq!(
        iterated
            .iter_prem
            .vars_bound
            .iter()
            .map(|var| var.id.node.as_str())
            .collect::<Vec<_>>(),
        vec!["input"]
    );
    assert_eq!(
        iterated
            .iter_prem
            .vars_bind
            .iter()
            .map(|var| var.id.node.as_str())
            .collect::<Vec<_>>(),
        vec!["output"]
    );
    assert_eq!(clause.node.expression, exp_output);
}

#[test]
fn conversion_traverses_relation_matches_paths_and_else_without_sibling_leaks() {
    let rule =
        |name: &str, input: ast::Exp, output: ast::Exp, premises: Vec<ast::Prem>, line: i64| {
            p4spec_rust::phrase! { node:
            ast::RuleKind {
                id: id(name, line),
                not_exp: Mixfix::Seq(vec![Mixfix::Arg(input), Mixfix::Arg(output)]),
                prems: premises,
            }, span:
            span(line) }
        };
    let debug_index = |value: bool, line: i64| {
        p4spec_rust::phrase! { node:
        ast::PremKind::Debug(ast::DebugPrem {
            exp: literal_index_exp(value, line),
        }), span:
        span(line) }
    };
    let match_rule = rule(
        "match_path",
        literal_index_exp(true, 10),
        literal_index_exp(true, 11),
        vec![],
        10,
    );
    let first_sibling = rule(
        "first_sibling",
        exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, 20),
        literal_index_exp(false, 22),
        vec![debug_index(false, 21)],
        20,
    );
    let second_sibling = rule(
        "second_sibling",
        exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, 30),
        literal_index_exp(false, 31),
        vec![],
        30,
    );
    let else_rule = rule(
        "else_path",
        literal_index_exp(true, 40),
        literal_index_exp(true, 42),
        vec![debug_index(false, 41)],
        40,
    );
    let spec = vec![p4spec_rust::phrase! { node:
    ast::DefKind::Rel(ast::Rel {
        id: id("relation", 1),
        not_typ: p4spec_rust::phrase! { node:
            Mixfix::Seq(vec![Mixfix::Arg(typ::bool()), Mixfix::Arg(typ::bool())]), span:
            span(1) },
        input_hint: InputHint::new(vec![0]),
        rule_groups: vec![
            p4spec_rust::phrase! { node: (id("match_group", 9), vec![match_rule]), span:  span(9) },
            p4spec_rust::phrase! { node:
                (id("sibling_group", 19), vec![first_sibling, second_sibling]), span:
                span(19) },
        ],
        else_group: Some(p4spec_rust::phrase! { node: (id("else_group", 39), else_rule), span:  span(39) }),
        hints: vec![],
    }), span:
    span(1) }];

    let converted = algo::convert(&spec).expect("guarded relation conversion");
    let p4spec_rust::lang::al::ast::DefKind::Rel(relation) = &converted[0].node else {
        panic!("expected relation definition");
    };
    let [match_group, sibling_group] = relation.rule_groups.as_slice() else {
        panic!("expected match and sibling groups in source order");
    };
    assert_eq!(match_group.span, span(9));
    assert_eq!(match_group.node.rule_match.exps_input[0].span, span(10));
    let [match_guard, match_source] = match_group.node.rule_match.prems.as_slice() else {
        panic!("expected a guard before the analyzed match premise");
    };
    assert_index_guard_span(match_guard, span(10));
    assert_eq!(match_source.span, span(10));
    assert!(match_group.node.rule_paths[0].prems.is_empty());

    let [first_path, second_path] = sibling_group.node.rule_paths.as_slice() else {
        panic!("expected both sibling paths in source order");
    };
    let [first_guard, first_source] = first_path.prems.as_slice() else {
        panic!("expected one path-local guard before the source premise");
    };
    assert_index_guard_span(first_guard, span(21));
    assert_eq!(first_source.span, span(21));
    assert!(matches!(first_source.node, ast::PremKind::Debug(_)));
    let [second_guard] = second_path.prems.as_slice() else {
        panic!("the first sibling must not guard the second sibling output");
    };
    assert_index_guard_span(second_guard, span(31));

    let else_group = relation.else_group.as_ref().expect("else group preserved");
    assert_eq!(else_group.span, span(39));
    assert_eq!(else_group.node.rule_match.exps_input[0].span, span(40));
    let [else_guard, else_source] = else_group.node.rule_path.prems.as_slice() else {
        panic!("expected else-path guard and source premise only");
    };
    assert_index_guard_span(else_guard, span(41));
    assert_eq!(else_source.span, span(41));
    assert!(matches!(else_source.node, ast::PremKind::Debug(_)));
}

#[test]
fn conversion_traverses_else_clauses_in_guard_order() {
    let typ_bool = typ::bool();
    let exp_argument = joint_iteration(&[("left", 50), ("right", 51)], ast::Iter::List, 50);
    let exp_output = joint_iteration(&[("left", 50), ("right", 51)], ast::Iter::List, 52);
    let prem_debug = p4spec_rust::phrase! { node:
    ast::PremKind::Debug(ast::DebugPrem {
        exp: literal_index_exp(false, 53),
    }), span:
    span(53) };
    let else_clause = p4spec_rust::phrase! { node:
    ast::ClauseKind {
        args: vec![exp_arg(exp_argument)],
        expression: exp_output.clone(),
        premises: vec![prem_debug],
    }, span:
    span(50) };
    let spec = vec![p4spec_rust::phrase! { node:
    ast::DefKind::FuncDec(ast::FuncDec {
        id: id("otherwise", 49),
        tparams: vec![],
        params: vec![p4spec_rust::phrase! { node:
            ast::ParamKind::Exp(typ::list(p4spec_rust::phrase! { node:
                ast::TypKind::Tuple(vec![typ_bool.clone(), typ_bool]), span:
                span(49) })), span:
            span(49) }],
        typ: p4spec_rust::phrase! { node: exp_output.note.clone(), span:  span(49) },
        clauses: vec![],
        else_clause: Some(else_clause),
        hints: vec![],
    }), span:
    span(49) }];

    let converted = algo::convert(&spec).expect("guarded else-clause conversion");
    let p4spec_rust::lang::al::ast::DefKind::FuncDec(function) = &converted[0].node else {
        panic!("expected function definition");
    };
    let clause = function
        .else_clause
        .as_ref()
        .expect("else clause preserved");
    let [guard, source] = clause.node.premises.as_slice() else {
        panic!("expected guard before the else-clause source premise");
    };
    assert_index_guard_span(guard, span(53));
    assert_eq!(source.span, span(53));
    assert!(matches!(source.node, ast::PremKind::Debug(_)));
    assert_eq!(clause.node.expression, exp_output);
    assert_eq!(clause.span, span(50));
}

#[test]
fn context_loads_type_and_metavariable_definitions() {
    let extern_id = id("extern_type", 1);
    let defined_id = id("defined_type", 2);
    let variable_id = id("value", 3);
    let bool_typ = p4spec_rust::phrase! { node: ast::TypKind::Bool, span:  span(2) };
    let def_typ =
        p4spec_rust::phrase! { node: ast::DefTypKind::Plain(bool_typ.clone()), span:  span(2) };
    let spec = vec![
        p4spec_rust::phrase! { node:
        ast::DefKind::ExternTyp(ast::ExternTyp {
            id: extern_id.clone(),
            hints: vec![],
        }), span:
        span(1) },
        p4spec_rust::phrase! { node:
        ast::DefKind::Typ(ast::TypDef {
            id: defined_id.clone(),
            tparams: vec![],
            def_typ: def_typ.clone(),
            hints: vec![],
        }), span:
        span(2) },
        p4spec_rust::phrase! { node:
        ast::DefKind::Var(ast::VarDef {
            id: variable_id.clone(),
            typ: bool_typ.clone(),
            hints: vec![],
        }), span:
        span(3) },
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
    let mut typ_second = typ::bool();
    typ_second.span = span(3);
    let mut bindings_l = Bindings::new();
    bindings_l.insert(id_first.clone(), Binding::Single(dim.clone()));
    let mut bindings_r = Bindings::new();
    bindings_r.insert(id_second, Binding::Single(Dim::new(typ_second, vec![])));

    let bindings = bind::union(bindings_l, bindings_r).expect("equivalent dimensions");

    assert_eq!(bindings.keys().next(), Some(&id_first));
    let Binding::Multiple(actual) = bindings.get(&id_first).expect("merged binding") else {
        panic!("expected a repeated binding");
    };
    assert!(actual.sub(&dim));
    assert!(dim.sub(actual));
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
    let expected = Dim::new(typ::bool(), vec![]);
    assert!(actual.sub(&expected));
    assert!(expected.sub(actual));
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
        .map(|exp| p4spec_rust::phrase! { node: ast::ArgKind::Exp(Box::new(exp)), span:  span(1) })
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
    let argument = p4spec_rust::phrase! { node: Mixfix::Arg(typ::bool()), span:  span(2) };
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

#[test]
fn multiple_binding_renames_repetitions_and_compares_them_in_occurrence_order() {
    let tuple = exp(
        ast::ExpKind::Tuple(vec![var_exp("x", 1), var_exp("x", 2), var_exp("x", 3)]),
        ast::TypKind::Tuple(vec![typ::bool(), typ::bool(), typ::bool()]),
        1,
    );
    let bindings = collect::collect_exp(&Context::new(), &tuple).expect("binding collection");
    let mut context = Context::new();
    let mut renames = multiple::RenameEnv::from_bindings(&bindings);

    let renamed = multiple::rename_exp(&mut context, &mut renames, &tuple);
    let side_conditions =
        multiple::generate_side_conditions(&bindings, &IterationContext::new(), &renames);

    let ast::ExpKind::Tuple(exps) = &renamed.node else {
        panic!("expected tuple binding");
    };
    let ids = exps
        .iter()
        .map(|exp| match &exp.node {
            ast::ExpKind::Var(id) => id,
            _ => panic!("expected variable binding"),
        })
        .collect::<Vec<_>>();
    assert_eq!(ids[0].node, "x");
    assert_ne!(ids[1].node, "x");
    assert_ne!(ids[2].node, "x");
    assert_ne!(ids[1].node, ids[2].node);
    assert_eq!(ids[1].span, span(2));
    assert_eq!(ids[2].span, span(3));

    let [side_condition] = side_conditions.as_slice() else {
        panic!("expected one equality side condition");
    };
    let ast::PremKind::If(if_prem) = &side_condition.node else {
        panic!("expected conditional premise");
    };
    let ast::ExpKind::Bin(_, ast::OpTyp::Bool, first, second) = &if_prem.exp.node else {
        panic!("expected ordered equality conjunction");
    };
    let compared_span = |exp: &ast::Exp| {
        let ast::ExpKind::Cmp(_, ast::OpTyp::Bool, _, exp_r) = &exp.node else {
            panic!("expected equality comparison");
        };
        let ast::ExpKind::Var(id) = &exp_r.node else {
            panic!("expected renamed right operand");
        };
        id.span.clone()
    };
    assert_eq!(compared_span(first), span(2));
    assert_eq!(compared_span(second), span(3));
}

#[test]
fn multiple_side_conditions_use_the_rename_environment_dimension() {
    let mut bindings = Bindings::new();
    bindings.insert(
        id("x", 1),
        Binding::Multiple(Dim::new(typ::bool(), vec![ast::Iter::List])),
    );
    let tuple = exp(
        ast::ExpKind::Tuple(vec![var_exp("x", 1), var_exp("x", 2)]),
        ast::TypKind::Tuple(vec![typ::bool(), typ::bool()]),
        1,
    );
    let mut context = Context::new();
    let mut renames = multiple::RenameEnv::from_bindings(&bindings);
    multiple::rename_exp(&mut context, &mut renames, &tuple);

    let premises =
        multiple::generate_side_conditions(&Bindings::new(), &IterationContext::new(), &renames);

    let [premise] = premises.as_slice() else {
        panic!("expected one repeated-binding premise");
    };
    let ast::PremKind::Iter(iterated) = &premise.node else {
        panic!("expected the collected binding dimension");
    };
    assert_eq!(iterated.iter_prem.iter, ast::Iter::List);
    assert!(matches!(iterated.prem.node, ast::PremKind::If(_)));
}

#[test]
fn partial_binding_preserves_expression_and_premise_iteration_dimensions() {
    let bool_value = exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, 2);
    let tuple = exp(
        ast::ExpKind::Tuple(vec![var_exp("x", 1), bool_value]),
        ast::TypKind::Tuple(vec![typ::bool(), typ::bool()]),
        1,
    );
    let iterated = exp(
        ast::ExpKind::Iter(
            Box::new(tuple),
            (
                ast::Iter::List,
                vec![ast::Var {
                    id: id("x", 1),
                    typ: typ::bool(),
                    iters: vec![],
                }],
            ),
        ),
        ast::TypKind::Iter(
            Box::new(p4spec_rust::phrase! { node:
            ast::TypKind::Tuple(vec![typ::bool(), typ::bool()]), span:
            span(1) }),
            ast::Iter::List,
        ),
        1,
    );
    let mut context = Context::new();
    let bindings = collect::collect_exp(&context, &iterated).expect("binding collection");
    let mut renames = partial::RenameEnv::new();

    let (_, renamed) = partial::rename_exp(
        &mut context,
        &bindings.domain(),
        &mut renames,
        IterationContext::new(),
        &iterated,
    )
    .expect("partial binding rename");
    let premises = partial::generate_prems(&context, &IterationContext::new(), &renames)
        .expect("partial binding premises");

    let ast::ExpKind::Iter(exp_inner, (ast::Iter::List, vars)) = &renamed.node else {
        panic!("expected iterated binding");
    };
    let ast::ExpKind::Tuple(exps) = &exp_inner.node else {
        panic!("expected tuple binding");
    };
    let ast::ExpKind::Var(id_rename) = &exps[1].node else {
        panic!("expected bound value to be renamed");
    };
    assert_eq!(vars.len(), 2);
    assert_eq!(vars[0].id.node, "x");
    assert_eq!(vars[1].id, *id_rename);

    let [premise] = premises.as_slice() else {
        panic!("expected one equality premise");
    };
    let ast::PremKind::Iter(iterated_prem) = &premise.node else {
        panic!("expected premise iteration");
    };
    assert_eq!(iterated_prem.iter_prem.iter, ast::Iter::List);
    assert_eq!(iterated_prem.iter_prem.vars_bound.len(), 1);
    assert_eq!(iterated_prem.iter_prem.vars_bound[0].id, *id_rename);
    let ast::PremKind::If(if_prem) = &iterated_prem.prem.node else {
        panic!("expected equality side condition");
    };
    let ast::ExpKind::Cmp(_, ast::OpTyp::Bool, exp_l, exp_r) = &if_prem.exp.node else {
        panic!("expected equality comparison");
    };
    assert!(matches!(&exp_l.node, ast::ExpKind::Var(id) if id == id_rename));
    assert!(matches!(exp_r.node, ast::ExpKind::Bool(true)));
}

#[test]
fn partial_binding_preserves_nested_iteration_order_and_dimensions() {
    let tuple_typ = p4spec_rust::phrase! { node: ast::TypKind::Tuple(vec![typ::bool(), typ::bool()]), span:  span(1) };
    let tuple = exp(
        ast::ExpKind::Tuple(vec![
            var_exp("x", 1),
            exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, 2),
        ]),
        tuple_typ.node.clone(),
        1,
    );
    let inner_typ = p4spec_rust::phrase! { node:
    ast::TypKind::Iter(Box::new(tuple_typ), ast::Iter::Opt), span:
    span(1) };
    let inner = exp(
        ast::ExpKind::Iter(
            Box::new(tuple),
            (
                ast::Iter::Opt,
                vec![ast::Var {
                    id: id("x", 1),
                    typ: typ::bool(),
                    iters: vec![],
                }],
            ),
        ),
        inner_typ.node.clone(),
        1,
    );
    let iterated = exp(
        ast::ExpKind::Iter(
            Box::new(inner),
            (
                ast::Iter::List,
                vec![ast::Var {
                    id: id("x", 1),
                    typ: typ::bool(),
                    iters: vec![ast::Iter::Opt],
                }],
            ),
        ),
        ast::TypKind::Iter(Box::new(inner_typ), ast::Iter::List),
        1,
    );
    let mut context = Context::new();
    let bindings = collect::collect_exp(&context, &iterated).expect("binding collection");
    let mut renames = partial::RenameEnv::new();

    let (_, renamed) = partial::rename_exp(
        &mut context,
        &bindings.domain(),
        &mut renames,
        IterationContext::new(),
        &iterated,
    )
    .expect("nested partial binding rename");
    let premises = partial::generate_prems(&context, &IterationContext::new(), &renames)
        .expect("nested partial binding premises");

    let ast::ExpKind::Iter(inner, (ast::Iter::List, outer_vars)) = &renamed.node else {
        panic!("expected outer list iteration");
    };
    let ast::ExpKind::Iter(tuple, (ast::Iter::Opt, inner_vars)) = &inner.node else {
        panic!("expected inner optional iteration");
    };
    let ast::ExpKind::Tuple(exps) = &tuple.node else {
        panic!("expected iterated tuple");
    };
    let ast::ExpKind::Var(id_rename) = &exps[1].node else {
        panic!("expected nested bound value rename");
    };
    assert_eq!(inner_vars.len(), 2);
    assert_eq!(outer_vars.len(), 2);
    assert_eq!(inner_vars[1].id, *id_rename);
    assert_eq!(outer_vars[1].id, *id_rename);

    let [premise] = premises.as_slice() else {
        panic!("expected one nested equality premise");
    };
    let ast::PremKind::Iter(outer) = &premise.node else {
        panic!("expected outer premise iteration");
    };
    assert_eq!(outer.iter_prem.iter, ast::Iter::List);
    assert_eq!(outer.iter_prem.vars_bound[0].id, *id_rename);
    let ast::PremKind::Iter(inner) = &outer.prem.node else {
        panic!("expected inner premise iteration");
    };
    assert_eq!(inner.iter_prem.iter, ast::Iter::Opt);
    assert_eq!(inner.iter_prem.vars_bound[0].id, *id_rename);
}

#[test]
fn partial_binding_rolls_back_context_and_renames_after_late_failure() {
    let initial = exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, 1);
    let mut context = Context::new();
    let initial_bindings = collect::collect_exp(&context, &initial).expect("binding collection");
    let mut renames = partial::RenameEnv::new();
    partial::rename_exp(
        &mut context,
        &initial_bindings.domain(),
        &mut renames,
        IterationContext::new(),
        &initial,
    )
    .expect("initial partial binding rename");
    let frees_before = context.frees.clone();
    let premise_count_before =
        partial::generate_prems(&context, &IterationContext::new(), &renames)
            .expect("initial premises")
            .len();

    let missing_typ = ast::TypKind::Var(id("Missing", 12), vec![]);
    let case = exp(
        ast::ExpKind::Case(Box::new(Mixfix::Arg(var_exp("y", 12)))),
        missing_typ.clone(),
        12,
    );
    let tuple = exp(
        ast::ExpKind::Tuple(vec![
            exp(ast::ExpKind::Bool(false), ast::TypKind::Bool, 11),
            case,
        ]),
        ast::TypKind::Tuple(vec![
            typ::bool(),
            p4spec_rust::phrase! { node: missing_typ, span:  span(12) },
        ]),
        11,
    );
    let bindings = collect::collect_exp(&context, &tuple).expect("binding collection");

    let error = partial::rename_exp(
        &mut context,
        &bindings.domain(),
        &mut renames,
        IterationContext::new(),
        &tuple,
    )
    .expect_err("undefined case type");

    assert_eq!(error.kind, AlgoErrorKind::UndefinedType);
    assert_eq!(error.span, span(12));
    assert_eq!(context.frees, frees_before);
    assert_eq!(
        partial::generate_prems(&context, &IterationContext::new(), &renames)
            .expect("rolled-back premises")
            .len(),
        premise_count_before
    );
}

#[test]
fn partial_case_and_list_bindings_generate_match_then_bind_premises_in_source_order() {
    let choice_id = id("Choice", 1);
    let choice_typ =
        p4spec_rust::phrase! { node: ast::TypKind::Var(choice_id.clone(), vec![]), span:  span(1) };
    let origin = p4spec_rust::phrase! { node: (choice_id.clone(), vec![]), span:  span(1) };
    let def_typ = p4spec_rust::phrase! { node:
    ast::DefTypKind::Variant(vec![
        (not_typ("A", 1), origin.clone(), vec![]),
        (not_typ("B", 1), origin, vec![]),
    ]), span:
    span(1) };
    let mut context = Context::new();
    context
        .tdenv
        .insert(choice_id, TypeDef::Defined(vec![], Box::new(def_typ)));

    let keyword = p4spec_rust::phrase! { node: Atom::Keyword("A".to_owned()), span:  span(2) };
    let case = exp(
        ast::ExpKind::Case(Box::new(Mixfix::Seq(vec![
            Mixfix::Atom(keyword),
            Mixfix::Arg(var_exp("y", 2)),
        ]))),
        choice_typ.node.clone(),
        2,
    );
    let list_typ = p4spec_rust::phrase! { node:
    ast::TypKind::Iter(Box::new(typ::bool()), ast::Iter::List), span:
    span(3) };
    let list = exp(
        ast::ExpKind::List(vec![var_exp("z", 3)]),
        list_typ.node.clone(),
        3,
    );
    let tuple = exp(
        ast::ExpKind::Tuple(vec![case, list]),
        ast::TypKind::Tuple(vec![choice_typ, list_typ]),
        2,
    );
    let bindings = collect::collect_exp(&context, &tuple).expect("binding collection");
    let mut renames = partial::RenameEnv::new();

    let (_, renamed) = partial::rename_exp(
        &mut context,
        &bindings.domain(),
        &mut renames,
        IterationContext::new(),
        &tuple,
    )
    .expect("partial binding rename");
    let premises = partial::generate_prems(&context, &IterationContext::new(), &renames)
        .expect("partial binding premises");

    let ast::ExpKind::Tuple(exps) = &renamed.node else {
        panic!("expected tuple binding");
    };
    assert!(matches!(exps[0].node, ast::ExpKind::Var(_)));
    assert!(matches!(exps[1].node, ast::ExpKind::Iter(_, _)));
    assert_eq!(premises.len(), 4);
    assert!(matches!(
        &premises[0].node,
        ast::PremKind::If(ast::IfPrem {
            exp: NotePhrase {
                node: ast::ExpKind::Match(_, ast::Pattern::Case(_)),
                ..
            }
        })
    ));
    assert!(matches!(
        &premises[1].node,
        ast::PremKind::Let(ast::LetPrem {
            exp_l: NotePhrase {
                node: ast::ExpKind::Case(_),
                ..
            },
            ..
        })
    ));
    assert!(matches!(
        &premises[2].node,
        ast::PremKind::If(ast::IfPrem {
            exp: NotePhrase {
                node: ast::ExpKind::Match(_, ast::Pattern::List(ast::ListPattern::Fixed(1))),
                ..
            }
        })
    ));
    assert!(matches!(
        &premises[3].node,
        ast::PremKind::Let(ast::LetPrem {
            exp_l: NotePhrase {
                node: ast::ExpKind::List(_),
                ..
            },
            ..
        })
    ));
}

#[test]
fn partial_upcast_binding_checks_subtype_before_binding_the_downcast_value() {
    let parent_id = id("Parent", 1);
    let child_id = id("Child", 1);
    let parent_typ =
        p4spec_rust::phrase! { node: ast::TypKind::Var(parent_id.clone(), vec![]), span:  span(1) };
    let child_typ =
        p4spec_rust::phrase! { node: ast::TypKind::Var(child_id.clone(), vec![]), span:  span(1) };
    let parent_origin = p4spec_rust::phrase! { node: (parent_id.clone(), vec![]), span:  span(1) };
    let child_origin = p4spec_rust::phrase! { node: (child_id.clone(), vec![]), span:  span(1) };
    let mut context = Context::new();
    context.tdenv.insert(
        parent_id,
        TypeDef::Defined(
            vec![],
            Box::new(p4spec_rust::phrase! { node:
            ast::DefTypKind::Variant(vec![
                (not_typ("A", 1), parent_origin.clone(), vec![]),
                (not_typ("B", 1), parent_origin, vec![]),
            ]), span:
            span(1) }),
        ),
    );
    context.tdenv.insert(
        child_id,
        TypeDef::Defined(
            vec![],
            Box::new(p4spec_rust::phrase! { node:
            ast::DefTypKind::Variant(vec![(not_typ("A", 1), child_origin, vec![])]), span:
            span(1) }),
        ),
    );
    let child_var = exp(ast::ExpKind::Var(id("child", 2)), child_typ.node.clone(), 2);
    let upcast = exp(
        ast::ExpKind::UpCast(parent_typ.clone(), Box::new(child_var)),
        parent_typ.node.clone(),
        2,
    );
    let bindings = collect::collect_exp(&context, &upcast).expect("binding collection");
    let mut renames = partial::RenameEnv::new();

    partial::rename_exp(
        &mut context,
        &bindings.domain(),
        &mut renames,
        IterationContext::new(),
        &upcast,
    )
    .expect("partial binding rename");
    let premises = partial::generate_prems(&context, &IterationContext::new(), &renames)
        .expect("partial binding premises");

    let [subtype, binding] = premises.as_slice() else {
        panic!("expected subtype and binding premises");
    };
    assert!(matches!(
        &subtype.node,
        ast::PremKind::If(ast::IfPrem {
            exp: NotePhrase {
                node: ast::ExpKind::Sub(_, typ, _),
                ..
            }
        }) if typ.syntax_eq(&child_typ)
    ));
    assert!(matches!(
        &binding.node,
        ast::PremKind::Let(ast::LetPrem {
            exp_l: NotePhrase {
                node: ast::ExpKind::Var(_),
                ..
            },
            exp_r: NotePhrase {
                node: ast::ExpKind::DownCast(typ, _),
                ..
            }
        }) if typ.syntax_eq(&child_typ)
    ));
}

#[test]
fn antiunification_populates_each_path_in_left_to_right_expression_order() {
    let tuple = |left: bool, right: bool, line: i64| {
        exp(
            ast::ExpKind::Tuple(vec![
                exp(ast::ExpKind::Bool(left), ast::TypKind::Bool, line),
                exp(ast::ExpKind::Bool(right), ast::TypKind::Bool, line + 1),
            ]),
            ast::TypKind::Tuple(vec![typ::bool(), typ::bool()]),
            line,
        )
    };
    let groups = vec![
        vec![tuple(true, false, 1), var_exp("shared", 3)],
        vec![tuple(false, true, 5), var_exp("shared", 7)],
    ];

    let (context, template, premises) =
        antiunify::antiunify(Context::new(), groups).expect("equivalent tuple inputs");

    assert_eq!(template.len(), 2);
    let ast::ExpKind::Tuple(items) = &template[0].node else {
        panic!("expected tuple template");
    };
    let template_ids = items
        .iter()
        .map(|item| match &item.node {
            ast::ExpKind::Var(id) => id,
            _ => panic!("expected fresh unifier"),
        })
        .collect::<Vec<_>>();
    assert_ne!(template_ids[0].node, template_ids[1].node);
    assert!(context.frees.contains(template_ids[0]));
    assert!(context.frees.contains(template_ids[1]));
    assert!(matches!(&template[1].node, ast::ExpKind::Var(id) if id.node == "shared"));

    let compared_values = |prems: &[ast::Prem]| {
        prems
            .iter()
            .map(|prem| {
                let ast::PremKind::If(if_prem) = &prem.node else {
                    panic!("expected equality premise");
                };
                let ast::ExpKind::Cmp(_, ast::OpTyp::Bool, _, exp_r) = &if_prem.exp.node else {
                    panic!("expected equality comparison");
                };
                let ast::ExpKind::Bool(value) = exp_r.node else {
                    panic!("expected original boolean expression");
                };
                value
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(premises.len(), 2);
    assert_eq!(compared_values(&premises[0]), vec![true, false]);
    assert_eq!(compared_values(&premises[1]), vec![false, true]);
}

#[test]
fn antiunification_freshness_avoids_collisions_within_each_operation() {
    let fresh_unifier = |context: Context| {
        let (_, template, _) = antiunify::antiunify(
            context,
            vec![
                vec![exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, 1)],
                vec![exp(ast::ExpKind::Bool(false), ast::TypKind::Bool, 2)],
            ],
        )
        .expect("equivalent boolean inputs");
        let ast::ExpKind::Var(id) = &template[0].node else {
            panic!("expected fresh unifier");
        };
        id.clone()
    };

    let id_first = fresh_unifier(Context::new());
    let mut context_collision = Context::new();
    context_collision.add_free(id_first.clone());
    let id_after_collision = fresh_unifier(context_collision);
    let id_independent = fresh_unifier(Context::new());

    assert_ne!(id_after_collision.node, id_first.node);
    assert_eq!(id_independent.node, id_first.node);
}

#[test]
fn antiunification_uses_runtime_equivalence_for_plain_type_aliases() {
    let alias_id = id("Flag", 1);
    let alias_typ =
        p4spec_rust::phrase! { node: ast::TypKind::Var(alias_id.clone(), vec![]), span:  span(1) };
    let mut context = Context::new();
    context.tdenv.insert(
        alias_id,
        TypeDef::Defined(
            vec![],
            Box::new(
                p4spec_rust::phrase! { node: ast::DefTypKind::Plain(typ::bool()), span:  span(1) },
            ),
        ),
    );
    let alias_value = exp(ast::ExpKind::Bool(true), alias_typ.node, 2);
    let bool_value = exp(ast::ExpKind::Bool(false), ast::TypKind::Bool, 3);

    let (_, template, premises) =
        antiunify::antiunify(context, vec![vec![alias_value], vec![bool_value]])
            .expect("plain alias is equivalent to its underlying type");

    assert!(matches!(template[0].node, ast::ExpKind::Var(_)));
    assert_eq!(
        premises.iter().map(Vec::len).collect::<Vec<_>>(),
        vec![1, 1]
    );
}

#[test]
fn conversion_preserves_rule_paths_and_populates_antiunified_inputs_in_order() {
    let tuple = |left: bool, right: bool, line: i64| {
        exp(
            ast::ExpKind::Tuple(vec![
                exp(ast::ExpKind::Bool(left), ast::TypKind::Bool, line),
                exp(ast::ExpKind::Bool(right), ast::TypKind::Bool, line + 1),
            ]),
            ast::TypKind::Tuple(vec![typ::bool(), typ::bool()]),
            line,
        )
    };
    let relation_not_typ = p4spec_rust::phrase! { node:
    Mixfix::Seq(vec![
        Mixfix::Arg(p4spec_rust::phrase! { node:
            ast::TypKind::Tuple(vec![typ::bool(), typ::bool()]), span:
            span(1) }),
        Mixfix::Arg(typ::bool()),
    ]), span:
    span(1) };
    let rule = |name: &str, input: ast::Exp, output: bool, line: i64| {
        p4spec_rust::phrase! { node:
        ast::RuleKind {
            id: id(name, line),
            not_exp: Mixfix::Seq(vec![
                Mixfix::Arg(input),
                Mixfix::Arg(exp(ast::ExpKind::Bool(output), ast::TypKind::Bool, line)),
            ]),
            prems: vec![],
        }, span:
        span(line) }
    };
    let rules_first = vec![
        rule("first", tuple(true, false, 2), true, 2),
        rule("second", tuple(false, true, 5), false, 5),
    ];
    let rules_second = vec![rule("third", tuple(true, true, 8), false, 8)];
    let spec = vec![p4spec_rust::phrase! { node:
    ast::DefKind::Rel(ast::Rel {
        id: id("relation", 1),
        not_typ: relation_not_typ,
        input_hint: InputHint::new(vec![0]),
        rule_groups: vec![
            p4spec_rust::phrase! { node: (id("first_group", 1), rules_first), span:  span(1) },
            p4spec_rust::phrase! { node: (id("second_group", 8), rules_second), span:  span(8) },
        ],
        else_group: None,
        hints: vec![],
    }), span:
    span(1) }];

    let analyzed = algo::convert(&spec).expect("convertible relation");

    let p4spec_rust::lang::al::ast::DefKind::Rel(relation) = &analyzed[0].node else {
        panic!("expected relation definition");
    };
    let [rule_group, second_group] = relation.rule_groups.as_slice() else {
        panic!("expected two rule groups");
    };
    assert_eq!(rule_group.node.id.node, "first_group");
    assert_eq!(second_group.node.id.node, "second_group");
    assert_eq!(second_group.node.rule_paths[0].id.node, "third");
    assert_eq!(
        rule_group
            .node
            .rule_paths
            .iter()
            .map(|path| path.id.node.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "second"]
    );
    let ast::ExpKind::Tuple(items) = &rule_group.node.rule_match.exps_signature[0].node else {
        panic!("expected tuple rule signature");
    };
    assert!(
        items
            .iter()
            .all(|item| matches!(item.node, ast::ExpKind::Var(_)))
    );
    let compared_values = |prems: &[ast::Prem]| {
        prems
            .iter()
            .map(|prem| {
                let ast::PremKind::If(if_prem) = &prem.node else {
                    panic!("expected populated equality premise");
                };
                let ast::ExpKind::Cmp(_, _, _, exp_r) = &if_prem.exp.node else {
                    panic!("expected equality comparison");
                };
                let ast::ExpKind::Bool(value) = exp_r.node else {
                    panic!("expected original boolean input");
                };
                value
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        compared_values(&rule_group.node.rule_paths[0].prems),
        vec![true, false]
    );
    assert_eq!(
        compared_values(&rule_group.node.rule_paths[1].prems),
        vec![false, true]
    );
    assert!(matches!(
        rule_group.node.rule_paths[0].exps_output[0].node,
        ast::ExpKind::Bool(true)
    ));
    assert!(matches!(
        rule_group.node.rule_paths[1].exps_output[0].node,
        ast::ExpKind::Bool(false)
    ));
}

#[test]
fn clause_analysis_orders_partial_then_repeated_then_source_premises() {
    let tuple_typ = p4spec_rust::phrase! { node:
    ast::TypKind::Tuple(vec![typ::bool(), typ::bool(), typ::bool()]), span:
    span(1) };
    let tuple = exp(
        ast::ExpKind::Tuple(vec![
            var_exp("x", 2),
            var_exp("x", 3),
            exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, 4),
        ]),
        tuple_typ.node.clone(),
        2,
    );
    let clause = p4spec_rust::phrase! { node:
    ast::ClauseKind {
        args: vec![p4spec_rust::phrase! { node: ast::ArgKind::Exp(Box::new(tuple)), span:  span(2) }],
        expression: var_exp("x", 5),
        premises: vec![p4spec_rust::phrase! { node:
            ast::PremKind::Debug(ast::DebugPrem {
                exp: exp(ast::ExpKind::Bool(false), ast::TypKind::Bool, 6),
            }), span:
            span(6) }],
    }, span:
    span(2) };
    let spec = vec![p4spec_rust::phrase! { node:
    ast::DefKind::FuncDec(ast::FuncDec {
        id: id("function", 1),
        tparams: vec![],
        params: vec![p4spec_rust::phrase! { node: ast::ParamKind::Exp(tuple_typ), span:  span(1) }],
        typ: typ::bool(),
        clauses: vec![clause],
        else_clause: None,
        hints: vec![],
    }), span:
    span(1) }];

    let analyzed = algo::convert(&spec).expect("convertible function");

    let p4spec_rust::lang::al::ast::DefKind::FuncDec(function) = &analyzed[0].node else {
        panic!("expected function definition");
    };
    let prems = &function.clauses[0].node.premises;
    assert_eq!(prems.len(), 3);
    assert!(matches!(
        &prems[0].node,
        ast::PremKind::If(ast::IfPrem {
            exp: NotePhrase {
                node: ast::ExpKind::Cmp(_, _, _, exp_r),
                ..
            }
        }) if matches!(exp_r.node, ast::ExpKind::Bool(true))
    ));
    assert!(matches!(
        &prems[1].node,
        ast::PremKind::If(ast::IfPrem {
            exp: NotePhrase {
                node: ast::ExpKind::Cmp(_, _, _, exp_r),
                ..
            }
        }) if matches!(exp_r.node, ast::ExpKind::Var(_))
    ));
    assert!(matches!(&prems[2].node, ast::PremKind::Debug(_)));
}

#[test]
fn otherwise_clauses_and_rules_reject_impure_premises_at_the_branch_span() {
    let impure_premise = |line: i64| {
        p4spec_rust::phrase! { node:
        ast::PremKind::If(ast::IfPrem {
            exp: exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, line),
        }), span:
        span(line) }
    };
    let else_clause = p4spec_rust::phrase! { node:
    ast::ClauseKind {
        args: vec![p4spec_rust::phrase! { node:
            ast::ArgKind::Exp(Box::new(var_exp("x", 10))), span:
            span(10) }],
        expression: var_exp("x", 10),
        premises: vec![impure_premise(11)],
    }, span:
    span(10) };
    let function_spec = vec![p4spec_rust::phrase! { node:
    ast::DefKind::FuncDec(ast::FuncDec {
        id: id("function", 9),
        tparams: vec![],
        params: vec![p4spec_rust::phrase! { node: ast::ParamKind::Exp(typ::bool()), span:  span(9) }],
        typ: typ::bool(),
        clauses: vec![],
        else_clause: Some(else_clause),
        hints: vec![],
    }), span:
    span(9) }];

    let function_error = algo::convert(&function_spec).expect_err("impure otherwise clause");
    assert_eq!(function_error.kind, AlgoErrorKind::ImpureElsePremises);
    assert_eq!(function_error.span, span(10));

    let relation_not_typ = p4spec_rust::phrase! { node: Mixfix::Arg(typ::bool()), span:  span(20) };
    let else_rule = p4spec_rust::phrase! { node:
    ast::RuleKind {
        id: id("else_rule", 21),
        not_exp: Mixfix::Arg(var_exp("input", 21)),
        prems: vec![impure_premise(22)],
    }, span:
    span(21) };
    let relation_spec = vec![p4spec_rust::phrase! { node:
    ast::DefKind::Rel(ast::Rel {
        id: id("relation", 20),
        not_typ: relation_not_typ,
        input_hint: InputHint::new(vec![0]),
        rule_groups: vec![],
        else_group: Some(p4spec_rust::phrase! { node: (id("else_group", 20), else_rule), span:  span(20) }),
        hints: vec![],
    }), span:
    span(20) }];

    let relation_error = algo::convert(&relation_spec).expect_err("impure otherwise rule");
    assert_eq!(relation_error.kind, AlgoErrorKind::ImpureElsePremises);
    assert_eq!(relation_error.span, span(21));
}

#[test]
fn conversion_rejects_overlapping_and_missing_variant_table_patterns() {
    let choice_id = id("Choice", 1);
    let choice_typ =
        p4spec_rust::phrase! { node: ast::TypKind::Var(choice_id.clone(), vec![]), span:  span(1) };
    let origin = p4spec_rust::phrase! { node: (choice_id.clone(), vec![]), span:  span(1) };
    let choice_def = p4spec_rust::phrase! { node:
    ast::DefKind::Typ(ast::TypDef {
        id: choice_id,
        tparams: vec![],
        def_typ: p4spec_rust::phrase! { node:
            ast::DefTypKind::Variant(vec![
                (not_typ("A", 1), origin.clone(), vec![]),
                (not_typ("B", 1), origin, vec![]),
            ]), span:
            span(1) },
        hints: vec![],
    }), span:
    span(1) };
    let table = |rows: Vec<ast::TableRow>, line: i64| {
        p4spec_rust::phrase! { node:
        ast::DefKind::TableDec(ast::TableDec {
            id: id("table", line),
            params: vec![p4spec_rust::phrase! { node:
                ast::ParamKind::Exp(choice_typ.clone()), span:
                span(line) }],
            typ: typ::bool(),
            rows,
            hints: vec![],
        }), span:
        span(line - 1) }
    };
    let row = |pattern: ast::Exp, line: i64| {
        p4spec_rust::phrase! { node:
        (
            vec![p4spec_rust::phrase! { node:
                ast::ArgKind::Exp(Box::new(pattern)), span:
                span(line) }],
            exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, line),
        ), span:
        span(line) }
    };
    let case_pattern = |name: &str, line: i64| {
        let keyword =
            p4spec_rust::phrase! { node: Atom::Keyword(name.to_owned()), span:  span(line) };
        let case = exp(
            ast::ExpKind::Case(Box::new(Mixfix::Atom(keyword))),
            choice_typ.node.clone(),
            line,
        );
        exp(
            ast::ExpKind::UpCast(choice_typ.clone(), Box::new(case)),
            choice_typ.node.clone(),
            line,
        )
    };
    let overlap_spec = vec![
        choice_def.clone(),
        table(
            vec![
                row(case_pattern("A", 10), 10),
                row(case_pattern("A", 11), 11),
            ],
            9,
        ),
    ];

    let overlap_error =
        algo::convert(&overlap_spec).expect_err("overlap takes precedence over missing");
    assert_eq!(overlap_error.kind, AlgoErrorKind::OverlappingTablePatterns);
    assert_eq!(overlap_error.span, span(8));

    let missing_spec = vec![choice_def, table(vec![row(case_pattern("A", 20), 20)], 19)];

    let missing_error = algo::convert(&missing_spec).expect_err("missing B variant row");
    assert_eq!(missing_error.kind, AlgoErrorKind::MissingTablePatterns);
    assert_eq!(missing_error.span, span(18));
}

#[test]
fn conversion_accepts_crossed_alias_table_rows_from_source() {
    let source = r#"
syntax typeIR
syntax typeId = text
syntax typedefTypeIR = TYPEDEF typeId typeIR
syntax intTypeIR = INT
syntax typeIR =
  | intTypeIR
  | typedefTypeIR

tbl dec $compat(typeIR, typeIR) : bool
tbl def $compat =
  | (INT, INT) => true
  | (TYPEDEF _ typeIR_l, typeIR_r) => true
  | (typeIR_l, TYPEDEF _ typeIR_r) => true
  | (_, _) => false
"#;
    let spec_el = parse_string(source).expect("parse crossed alias table");
    let spec_il = elaborate::elaborate(&spec_el).expect("elaborate crossed alias table");

    algo::convert(&spec_il).expect("pinned conversion accepts source-distinct alias rows");
}

#[test]
fn conversion_preserves_definition_clause_and_table_row_order() {
    let choice_id = id("Choice", 2);
    let choice_typ =
        p4spec_rust::phrase! { node: ast::TypKind::Var(choice_id.clone(), vec![]), span:  span(2) };
    let origin = p4spec_rust::phrase! { node: (choice_id.clone(), vec![]), span:  span(2) };
    let choice_def = p4spec_rust::phrase! { node:
    ast::DefKind::Typ(ast::TypDef {
        id: choice_id,
        tparams: vec![],
        def_typ: p4spec_rust::phrase! { node:
            ast::DefTypKind::Variant(vec![
                (not_typ("A", 2), origin.clone(), vec![]),
                (not_typ("B", 2), origin, vec![]),
            ]), span:
            span(2) },
        hints: vec![],
    }), span:
    span(2) };
    let clause = |name: &str, line: i64| {
        p4spec_rust::phrase! { node:
        ast::ClauseKind {
            args: vec![p4spec_rust::phrase! { node:
                ast::ArgKind::Exp(Box::new(var_exp(name, line))), span:
                span(line) }],
            expression: var_exp(name, line),
            premises: vec![],
        }, span:
        span(line) }
    };
    let function_def = p4spec_rust::phrase! { node:
    ast::DefKind::FuncDec(ast::FuncDec {
        id: id("function", 3),
        tparams: vec![],
        params: vec![p4spec_rust::phrase! { node: ast::ParamKind::Exp(typ::bool()), span:  span(3) }],
        typ: typ::bool(),
        clauses: vec![clause("first_clause", 4), clause("second_clause", 5)],
        else_clause: None,
        hints: vec![],
    }), span:
    span(3) };
    let row = |name: &str, value: bool, line: i64| {
        let pattern = exp(
            ast::ExpKind::Var(id(name, line)),
            choice_typ.node.clone(),
            line,
        );
        p4spec_rust::phrase! { node:
        (
            vec![p4spec_rust::phrase! { node:
                ast::ArgKind::Exp(Box::new(pattern)), span:
                span(line) }],
            literal_index_exp(value, line),
        ), span:
        span(line) }
    };
    let table_def = p4spec_rust::phrase! { node:
    ast::DefKind::TableDec(ast::TableDec {
        id: id("table", 6),
        params: vec![p4spec_rust::phrase! { node:
            ast::ParamKind::Exp(choice_typ.clone()), span:
            span(6) }],
        typ: typ::bool(),
        rows: vec![row("specific", true, 7), row("_closer", false, 8)],
        hints: vec![],
    }), span:
    span(6) };
    let variable_def = p4spec_rust::phrase! { node:
    ast::DefKind::Var(ast::VarDef {
        id: id("variable", 9),
        typ: typ::bool(),
        hints: vec![],
    }), span:
    span(9) };
    let extern_relation_def = p4spec_rust::phrase! { node:
    ast::DefKind::ExternRel(ast::ExternRel {
        id: id("external_relation", 10),
        not_typ: p4spec_rust::phrase! { node: Mixfix::Arg(typ::bool()), span:  span(10) },
        input_hint: InputHint::new(vec![0]),
        hints: vec![],
    }), span:
    span(10) };
    let extern_dec_def = p4spec_rust::phrase! { node:
    ast::DefKind::ExternDec(ast::ExternDec {
        id: id("external_dec", 11),
        tparams: vec![],
        params: vec![p4spec_rust::phrase! { node: ast::ParamKind::Exp(typ::bool()), span:  span(11) }],
        typ: typ::bool(),
        hints: vec![],
    }), span:
    span(11) };
    let builtin_dec_def = p4spec_rust::phrase! { node:
    ast::DefKind::BuiltinDec(ast::BuiltinDec {
        id: id("builtin_dec", 12),
        tparams: vec![],
        params: vec![p4spec_rust::phrase! { node: ast::ParamKind::Exp(typ::bool()), span:  span(12) }],
        typ: typ::bool(),
        hints: vec![],
    }), span:
    span(12) };
    let spec = vec![
        p4spec_rust::phrase! { node:
        ast::DefKind::ExternTyp(ast::ExternTyp {
            id: id("external", 1),
            hints: vec![],
        }), span:
        span(1) },
        variable_def,
        extern_relation_def,
        choice_def,
        extern_dec_def,
        builtin_dec_def,
        function_def,
        table_def,
    ];

    let analyzed = algo::convert(&spec).expect("ordered specification");

    let definition_ids = analyzed
        .iter()
        .map(|def| match &def.node {
            p4spec_rust::lang::al::ast::DefKind::ExternTyp(def) => def.id.node.as_str(),
            p4spec_rust::lang::al::ast::DefKind::Var(def) => def.id.node.as_str(),
            p4spec_rust::lang::al::ast::DefKind::ExternRel(def) => def.id.node.as_str(),
            p4spec_rust::lang::al::ast::DefKind::Rel(def) => def.id.node.as_str(),
            p4spec_rust::lang::al::ast::DefKind::Typ(def) => def.id.node.as_str(),
            p4spec_rust::lang::al::ast::DefKind::ExternDec(def) => def.id.node.as_str(),
            p4spec_rust::lang::al::ast::DefKind::BuiltinDec(def) => def.id.node.as_str(),
            p4spec_rust::lang::al::ast::DefKind::FuncDec(def) => def.id.node.as_str(),
            p4spec_rust::lang::al::ast::DefKind::TableDec(def) => def.id.node.as_str(),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        definition_ids,
        vec![
            "external",
            "variable",
            "external_relation",
            "Choice",
            "external_dec",
            "builtin_dec",
            "function",
            "table",
        ]
    );

    let p4spec_rust::lang::al::ast::DefKind::FuncDec(function) = &analyzed[6].node else {
        panic!("expected function definition");
    };
    let clause_ids = function
        .clauses
        .iter()
        .map(|clause| {
            let ast::ArgKind::Exp(exp) = &clause.node.args[0].node else {
                panic!("expected expression argument");
            };
            let ast::ExpKind::Var(id) = &exp.node else {
                panic!("expected variable argument");
            };
            id.node.as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(clause_ids, vec!["first_clause", "second_clause"]);

    let p4spec_rust::lang::al::ast::DefKind::TableDec(table) = &analyzed[7].node else {
        panic!("expected table definition");
    };
    let row_ids = table
        .table_rows
        .iter()
        .map(|row| {
            let ast::ExpKind::Var(id) = &row.node.exps_signature[0].node else {
                panic!("expected variable signature");
            };
            id.node.as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(row_ids, vec!["specific", "_closer"]);
    assert_eq!(
        table
            .table_rows
            .iter()
            .map(|row| row.span.clone())
            .collect::<Vec<_>>(),
        vec![span(7), span(8)]
    );
    assert!(table.table_rows.iter().all(|row| {
        row.node.prems.is_empty() && matches!(row.node.exp.node, ast::ExpKind::Idx(_, _))
    }));
}
