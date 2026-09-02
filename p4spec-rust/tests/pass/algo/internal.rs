use crate::{
    lang::{
        al::ast as ast_al,
        common::{
            Id,
            ds::set::IdSet,
            notation::{atom::Atom, mixfix::Mixfix},
            source::{NotePhrase, Position, Span},
        },
        hints::input::InputHint,
        il::ast,
        traits::eq::SyntaxEq,
        xl,
    },
    pass::algo::{
        self, AlgoErrorKind,
        binding::{
            antiunify,
            bind::{BEnv, Binding},
            collect,
            context::Context,
            dimension,
            iteration::{ICtx, Iteration},
            multiple, partial,
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
    crate::phrase! { node: name.to_owned(), span:  span(line) }
}

fn benv_domain(benv: &BEnv) -> IdSet {
    benv.iter().map(|(id, _)| id.clone()).collect()
}

fn exp(kind: ast::ExpKind, note: ast::TypKind, line: i64) -> ast::Exp {
    crate::note_phrase! { node: kind, note:  note, span:  span(line) }
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
    crate::phrase! { node: ast::ArgKind::Exp(Box::new(exp)), span:  span }
}

fn if_prem(exp: ast::Exp) -> ast::Prem {
    let span = exp.span.clone();
    crate::phrase! { node: ast::PremKind::If(ast::IfPrem { exp }), span:  span }
}

fn as_if_prem_al(prem_il: &ast::Prem) -> ast_al::Prem {
    let ast::PremKind::If(if_prem_il) = &prem_il.node else {
        panic!("expected conditional IL premise");
    };
    crate::phrase! {
        node: ast_al::PremKind::If(ast_al::IfPrem {
            exp: if_prem_il.exp.clone(),
        }),
        span: prem_il.span.clone(),
    }
}

fn function_spec(
    params: Vec<ast::Typ>,
    args: Vec<ast::Exp>,
    expression: ast::Exp,
    premises: Vec<ast::Prem>,
) -> ast::Spec {
    let typ = ast::typ_from_note(&expression.note, expression.span.clone());
    let clause = crate::phrase! { node:
    ast::ClauseKind {
        args: args.into_iter().map(exp_arg).collect(),
        expression,
        premises,
    }, span:
    span(1) };
    let params = params
        .into_iter()
        .map(|typ| crate::phrase! { node: ast::ParamKind::Exp(typ), span:  span(1) })
        .collect();
    vec![crate::phrase! { node:
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
    let typ_tuple = crate::phrase! { node: ast::TypKind::Tuple(vec![typ_bool; names.len()]), span:  span(line) };
    let exp_inner = exp(ast::ExpKind::Tuple(exps), typ_tuple.node.clone(), line);
    exp(
        ast::ExpKind::Iter(Box::new(exp_inner), (iter, vars)),
        ast::TypKind::Iter(Box::new(typ_tuple), iter),
        line,
    )
}

fn dimension_exp(name: &str, iter: ast::Iter, line: i64) -> ast::Exp {
    crate::lang::il::var::as_exp(
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

fn assert_index_guard_span(prem: &ast_al::Prem, expected_span: Span) {
    assert_eq!(prem.span, expected_span);
    let ast_al::PremKind::If(if_prem) = &prem.node else {
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

fn function_clause(spec: &crate::lang::al::ast::Spec) -> &ast_al::Clause {
    let crate::lang::al::ast::DefKind::FuncDec(function) = &spec[0].node else {
        panic!("expected function definition");
    };
    &function.clauses[0]
}

fn not_typ(name: &str, line: i64) -> ast::NotTyp {
    let atom = crate::phrase! { node: Atom::Keyword(name.to_owned()), span:  span(line) };
    crate::phrase! { node: Mixfix::Atom(atom), span:  span(line) }
}

fn pattern_set(names: &[&str]) -> PatternSet {
    names
        .iter()
        .enumerate()
        .map(|(index, name)| not_typ(name, index as i64 + 1))
        .collect()
}

#[path = "binding.rs"]
mod binding;
#[path = "sidecondition.rs"]
mod sidecondition;
