use p4spec_rust::{
    lang::{
        common::source::{Position, Span},
        el,
        hints::alter::{AlterationError, AlterationHint, Hole},
        il,
        pl::ast as pl,
        sl::ast as sl,
    },
    pass::prose::{self, ProseErrorKind},
};

fn span(name: &str, column: usize) -> Span {
    Span::new(Position::new(name, 0, column), Position::new(name, 0, column))
}

fn id(name: &str) -> il::ast::Id {
    p4spec_rust::phrase! { node: name.to_owned(), span: span(name, 0) }
}

fn typ_bool() -> il::ast::Typ {
    p4spec_rust::phrase! { node: il::ast::TypKind::Bool, span: span("type", 0) }
}

fn exp_bool(value: bool, span: Span) -> il::ast::Exp {
    p4spec_rust::note_phrase! {
        node: il::ast::ExpKind::Bool(value),
        note: il::ast::TypKind::Bool,
        span: span,
    }
}

fn exp_var(name: &str, span: Span) -> il::ast::Exp {
    p4spec_rust::note_phrase! {
        node: il::ast::ExpKind::Var(id(name)),
        note: il::ast::TypKind::Bool,
        span: span,
    }
}

fn var(name: &str) -> il::ast::Var {
    il::ast::Var { id: id(name), typ: typ_bool(), iters: Vec::new() }
}

fn exp_call(name: &str, arg_exp: il::ast::Exp, span: Span) -> il::ast::Exp {
    exp_call_args(name, vec![arg_exp], span)
}

fn exp_call_args(name: &str, args: Vec<il::ast::Exp>, span: Span) -> il::ast::Exp {
    p4spec_rust::note_phrase! {
        node: il::ast::ExpKind::Call(
            id(name),
            Vec::new(),
            args
                .into_iter()
                .map(|exp| p4spec_rust::phrase! {
                    node: il::ast::ArgKind::Exp(Box::new(exp)),
                    span: span.clone(),
                })
                .collect(),
        ),
        note: il::ast::TypKind::Bool,
        span: span,
    }
}

fn prose_in_hint(hole: el::ast::Hole, span: Span) -> sl::Hint {
    (id("prose_in"), p4spec_rust::phrase! { node: el::ast::ExpKind::Hole(hole), span: span })
}

fn extern_func(name: &str, hints: Vec<sl::Hint>) -> sl::Def {
    p4spec_rust::phrase! {
        node: sl::DefKind::MetaFunc(sl::MetaFuncDef::Extern(sl::ExternFunc {
            id: id(name),
            tparams: Vec::new(),
            params: Vec::new(),
            typ: typ_bool(),
            hints,
        })),
        span: span("extern", 0),
    }
}

fn return_instr(value: bool, span: Span) -> sl::Instr {
    p4spec_rust::phrase! {
        node: sl::InstrKind::Return(sl::ReturnInstr {
            exp: exp_bool(value, span.clone()),
        }),
        span: span,
    }
}

fn defined_func(block: sl::Block) -> sl::Def {
    p4spec_rust::phrase! {
        node: sl::DefKind::MetaFunc(sl::MetaFuncDef::Defined(sl::DefinedFunc {
            id: id("f"),
            tparams: Vec::new(),
            params: Vec::new(),
            typ: typ_bool(),
            block,
            block_else: None,
            hints: Vec::new(),
        })),
        span: span("function", 0),
    }
}

fn converted_func(block: sl::Block) -> pl::DefinedFunc {
    let mut spec_pl = prose::convert(vec![defined_func(block)]).unwrap();
    let def_pl = spec_pl.pop().unwrap();
    match def_pl.node.node {
        pl::DefKind::MetaFunc(pl::MetaFuncDef::Defined(def_func_pl)) => def_func_pl,
        def_kind_pl => panic!("expected defined function, got {def_kind_pl:?}"),
    }
}

#[test]
fn test_multiple_group_instructions_become_backtrack_arms() {
    let span_a = span("arms", 1);
    let span_b = span("arms", 4);
    let def_func_pl = converted_func(vec![
        return_instr(true, span_a.clone()),
        return_instr(false, span_b.clone()),
    ]);

    assert_eq!(def_func_pl.block.len(), 1);
    let instr_pl = &def_func_pl.block[0];
    assert_eq!(instr_pl.node.span, Span::over(&[span_a, span_b]));
    let pl::InstrKind::Tier(pl::TierInstr {
        tier: pl::InstrGroup::Backtrack(pl::BacktrackGroupInstr { blocks }),
    }) = &instr_pl.node.node
    else {
        panic!("expected backtracking alternatives, got {instr_pl:?}");
    };
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].len(), 1);
    assert_eq!(blocks[1].len(), 1);
}

#[test]
fn test_let_and_debug_precede_their_converted_continuations() {
    let span_let = span("let", 0);
    let instr_let = p4spec_rust::phrase! {
        node: sl::InstrKind::Let(sl::LetInstr {
            exp_l: exp_var("x", span_let.clone()),
            exp_r: exp_bool(true, span_let.clone()),
            iter_instrs: Vec::new(),
            block: vec![
                return_instr(true, span("let", 2)),
                return_instr(false, span("let", 3)),
            ],
        }),
        span: span_let,
    };
    let def_func_pl = converted_func(vec![instr_let]);
    assert!(matches!(def_func_pl.block[0].node.node, pl::InstrKind::Let(_)));
    assert!(matches!(
        def_func_pl.block[1].node.node,
        pl::InstrKind::Tier(pl::TierInstr { tier: pl::InstrGroup::Backtrack(_) })
    ));

    let span_debug = span("debug", 0);
    let instr_debug = p4spec_rust::phrase! {
        node: sl::InstrKind::Debug(sl::DebugInstr {
            exp: exp_bool(true, span_debug.clone()),
            instr: Box::new(return_instr(false, span("debug", 2))),
        }),
        span: span_debug,
    };
    let def_func_pl = converted_func(vec![instr_debug]);
    assert!(matches!(def_func_pl.block[0].node.node, pl::InstrKind::Debug(_)));
    assert!(matches!(
        def_func_pl.block[1].node.node,
        pl::InstrKind::Tier(pl::TierInstr { tier: pl::InstrGroup::Return(_) })
    ));
}

#[test]
fn test_group_in_group_body_reports_the_instruction_span() {
    let span_group = span("invalid-group", 3);
    let instr_group = p4spec_rust::phrase! {
        node: sl::InstrKind::Group(sl::GroupInstr {
            id: id("g"),
            rel_signature: sl::RelSignature {
                not_typ: p4spec_rust::phrase! {
                    node: p4spec_rust::lang::common::notation::mixfix::Mixfix::Arg(typ_bool()),
                    span: span("signature", 0),
                },
                input_hint: p4spec_rust::lang::hints::input::InputHint::new(vec![0]),
            },
            exps: vec![exp_bool(true, span_group.clone())],
            block: Vec::new(),
        }),
        span: span_group.clone(),
    };

    let error = prose::convert(vec![defined_func(vec![instr_group])]).unwrap_err();
    assert_eq!(error.kind, ProseErrorKind::InvalidGroupTier);
    assert_eq!(error.span, span_group);
}

#[test]
fn test_call_uses_hints_loaded_from_the_original_spec() {
    let span_call = span("call", 4);
    let def_func = defined_func(vec![p4spec_rust::phrase! {
        node: sl::InstrKind::Return(sl::ReturnInstr {
            exp: exp_call(
                "g",
                exp_bool(true, span("argument", 0)),
                span_call.clone(),
            ),
        }),
        span: span_call,
    }]);
    let spec_pl = prose::convert(vec![
        extern_func("g", vec![prose_in_hint(el::ast::Hole::Next, span("hint", 0))]),
        def_func,
    ])
    .unwrap();
    let pl::DefKind::MetaFunc(pl::MetaFuncDef::Defined(def_func_pl)) = &spec_pl[1].node.node else {
        panic!("expected defined function");
    };
    let pl::InstrKind::Tier(pl::TierInstr {
        tier: pl::InstrGroup::Return(pl::ReturnGroupInstr { exp: exp_pl }),
    }) = &def_func_pl.block[0].node.node
    else {
        panic!("expected return instruction");
    };
    assert_eq!(exp_pl.hints.prose_in, Some(AlterationHint::Hole(Hole::Next)));
}

#[test]
fn test_invalid_call_hint_reports_the_call_span() {
    let span_call = span("invalid-call", 7);
    let def_func = defined_func(vec![p4spec_rust::phrase! {
        node: sl::InstrKind::Return(sl::ReturnInstr {
            exp: exp_call(
                "g",
                exp_bool(true, span("argument", 0)),
                span_call.clone(),
            ),
        }),
        span: span_call.clone(),
    }]);
    let error = prose::convert(vec![
        extern_func("g", vec![prose_in_hint(el::ast::Hole::Num(1), span("hint", 0))]),
        def_func,
    ])
    .unwrap_err();

    assert_eq!(
        error.kind,
        ProseErrorKind::Alteration(AlterationError::IndexOutOfBounds { index: 1, item_count: 1 })
    );
    assert_eq!(error.span, span_call);
}

#[test]
fn test_nested_calls_expand_left_to_right_and_preserve_outer_call() {
    let exp_inner_a =
        exp_call("inner_a", exp_bool(true, span("inner-a-arg", 0)), span("inner-a", 0));
    let exp_inner_b =
        exp_call("inner_b", exp_bool(false, span("inner-b-arg", 0)), span("inner-b", 0));
    let span_outer = span("outer", 0);
    let exp_outer = exp_call_args("outer", vec![exp_inner_a, exp_inner_b], span_outer.clone());
    let def_func_pl = converted_func(vec![p4spec_rust::phrase! {
        node: sl::InstrKind::Return(sl::ReturnInstr { exp: exp_outer }),
        span: span_outer,
    }]);

    assert_eq!(def_func_pl.block.len(), 3);
    let mut ids_bound = Vec::new();
    let mut ids_called = Vec::new();
    for instr_pl in &def_func_pl.block[..2] {
        let pl::InstrKind::Let(pl::LetInstr { exp_l, exp_r, .. }) = &instr_pl.node.node else {
            panic!("expected expanded let, got {instr_pl:?}");
        };
        let pl::ExpKind::Var(id_bound) = &exp_l.node.node else {
            panic!("expected fresh variable binding");
        };
        let pl::ExpKind::Call(id_called, _, _) = &exp_r.node.node else {
            panic!("expected lifted call");
        };
        ids_bound.push(id_bound.node.clone());
        ids_called.push(id_called.node.clone());
    }
    assert_eq!(ids_called, vec!["inner_a", "inner_b"]);

    let pl::InstrKind::Tier(pl::TierInstr {
        tier: pl::InstrGroup::Return(pl::ReturnGroupInstr { exp: exp_outer_pl }),
    }) = &def_func_pl.block[2].node.node
    else {
        panic!("expected final return");
    };
    let pl::ExpKind::Call(id_outer, _, args_outer) = &exp_outer_pl.node.node else {
        panic!("expected outer call");
    };
    assert_eq!(id_outer.node, "outer");
    let ids_used = args_outer
        .iter()
        .map(|arg| match &arg.node {
            pl::ArgKind::Exp(exp) => match &exp.node.node {
                pl::ExpKind::Var(id) => id.node.clone(),
                exp_kind => panic!("expected variable argument, got {exp_kind:?}"),
            },
            arg_kind => panic!("expected expression argument, got {arg_kind:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(ids_used, ids_bound);
}

#[test]
fn test_instruction_iterator_local_call_stays_in_scope() {
    let span_let = span("iter-local", 0);
    let exp_local_call = exp_call("local", exp_var("x", span("x-use", 0)), span("local", 0));
    let exp_lifted_call =
        exp_call("independent", exp_bool(true, span("independent-arg", 0)), span("independent", 0));
    let exp_r = p4spec_rust::note_phrase! {
        node: il::ast::ExpKind::Tuple(vec![exp_local_call, exp_lifted_call]),
        note: il::ast::TypKind::Tuple(vec![typ_bool(), typ_bool()]),
        span: span_let.clone(),
    };
    let instr_let = p4spec_rust::phrase! {
        node: sl::InstrKind::Let(sl::LetInstr {
            exp_l: exp_var("result", span_let.clone()),
            exp_r,
            iter_instrs: vec![il::ast::PremIter {
                iter: il::ast::Iter::List,
                vars_bound: vec![var("x")],
                vars_bind: vec![var("result")],
            }],
            block: vec![return_instr(true, span("body", 0))],
        }),
        span: span_let,
    };

    let def_func_pl = converted_func(vec![instr_let]);
    assert_eq!(def_func_pl.block.len(), 3);

    let pl::InstrKind::Let(pl::LetInstr { exp_r, .. }) = &def_func_pl.block[0].node.node else {
        panic!("expected independent call to be lifted");
    };
    assert!(matches!(&exp_r.node.node, pl::ExpKind::Call(id, _, _) if id.node == "independent"));

    let pl::InstrKind::Let(pl::LetInstr { exp_r, iter_instrs, .. }) =
        &def_func_pl.block[1].node.node
    else {
        panic!("expected original iterated let");
    };
    assert_eq!(iter_instrs.len(), 1);
    let pl::ExpKind::Tuple(exps) = &exp_r.node.node else {
        panic!("expected tuple right-hand side");
    };
    assert!(matches!(&exps[0].node.node, pl::ExpKind::Call(id, _, _) if id.node == "local"));
    assert!(matches!(exps[1].node.node, pl::ExpKind::Var(_)));
}

#[test]
fn test_expression_iterator_lift_preserves_dimensions_and_outer_use() {
    let var_x = var("x");
    let exp_call = exp_call("each", exp_var("x", span("x-inner", 0)), span("each", 0));
    let exp_inner = p4spec_rust::note_phrase! {
        node: il::ast::ExpKind::Tuple(vec![exp_call, exp_var("x", span("x-sibling", 0))]),
        note: il::ast::TypKind::Tuple(vec![typ_bool(), typ_bool()]),
        span: span("iteration-inner", 0),
    };
    let exp_iter = p4spec_rust::note_phrase! {
        node: il::ast::ExpKind::Iter(
            Box::new(exp_inner),
            (il::ast::Iter::List, vec![var_x.clone()]),
        ),
        note: il::ast::TypKind::Iter(
            Box::new(p4spec_rust::phrase! {
                node: il::ast::TypKind::Tuple(vec![typ_bool(), typ_bool()]),
                span: span("tuple-type", 0),
            }),
            il::ast::Iter::List,
        ),
        span: span("iteration", 0),
    };
    let def_func_pl = converted_func(vec![p4spec_rust::phrase! {
        node: sl::InstrKind::Return(sl::ReturnInstr { exp: exp_iter }),
        span: span("return", 0),
    }]);

    let pl::InstrKind::Let(pl::LetInstr { exp_l, iter_instrs, .. }) =
        &def_func_pl.block[0].node.node
    else {
        panic!("expected lifted iterator call");
    };
    let pl::ExpKind::Var(id_fresh) = &exp_l.node.node else {
        panic!("expected fresh let variable");
    };
    assert_eq!(iter_instrs.len(), 1);
    assert_eq!(iter_instrs[0].iter, il::ast::Iter::List);
    assert_eq!(iter_instrs[0].vars_bound, vec![var_x.clone()]);
    assert_eq!(iter_instrs[0].vars_bind[0].id.node, id_fresh.node);

    let pl::InstrKind::Tier(pl::TierInstr {
        tier: pl::InstrGroup::Return(pl::ReturnGroupInstr { exp }),
    }) = &def_func_pl.block[1].node.node
    else {
        panic!("expected return after lifted call");
    };
    let pl::ExpKind::Iter(exp_inner, (_, vars)) = &exp.node.node else {
        panic!("expected iterated fresh result");
    };
    let pl::ExpKind::Tuple(exps) = &exp_inner.node.node else {
        panic!("expected iterated tuple");
    };
    assert!(matches!(&exps[0].node.node, pl::ExpKind::Var(id) if id.node == id_fresh.node));
    assert!(matches!(&exps[1].node.node, pl::ExpKind::Var(id) if id.node == "x"));
    assert_eq!(vars.len(), 2);
    assert_eq!(vars[0].id.node, id_fresh.node);
    assert_eq!(vars[1], var_x);
}
