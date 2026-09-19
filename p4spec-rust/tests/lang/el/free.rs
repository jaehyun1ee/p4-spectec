use super::*;

#[test]
fn test_free_expression_ids_ignore_source_spans_and_render_in_source_order() {
    let exp_bin = exp(
        ExpKind::Bin(
            Box::new(exp(ExpKind::Id(id("left", "left.watsup")), "left.watsup")),
            BinOp::Num(p4spec_rust::lang::common::prim::num::BinOp::Add),
            Box::new(exp(ExpKind::Id(id("right", "right.watsup")), "right.watsup")),
        ),
        "root.watsup",
    );

    assert_eq!(exp_bin.free(), ids(&["left", "right"]));
    assert_eq!(Print::to_string(&exp_bin), "left + right");
}
#[test]
fn test_free_collection_covers_paths_calls_premises_and_definition_bodies() {
    let id_exp = |name| exp(ExpKind::Id(id(name, "different-source.watsup")), "expr.watsup");
    let path = p4spec_rust::phrase! { node: ast::PathKind::Slice(
        Box::new(p4spec_rust::phrase! {
            node:
            ast::PathKind::Idx(
                Box::new(p4spec_rust::phrase! {
                    node: ast::PathKind::Root,
                    span: span("path.watsup"),
                }),
                Box::new(id_exp("index")),
            ),
            span: span("path.watsup"),
        }),
        Box::new(id_exp("low")),
        Box::new(id_exp("high")),
    ), span: span("path.watsup") };
    let call = exp(
        ExpKind::Call(
            id("defined", "definition.watsup"),
            vec![bool_typ()],
            vec![
                p4spec_rust::phrase! {
                    node: ast::ArgKind::Def(id("not_free", "arg.watsup")),
                    span: span("arg.watsup"),
                },
                p4spec_rust::phrase! {
                    node: ast::ArgKind::Exp(Box::new(id_exp("argument"))),
                    span: span("arg.watsup"),
                },
            ],
        ),
        "call.watsup",
    );
    let exp_upd =
        exp(ExpKind::Upd(Box::new(call), path, Box::new(id_exp("field"))), "update.watsup");
    let iteration = prem(ast::PremKind::Iter(ast::IterPrem {
        prem: Box::new(prem(ast::PremKind::Var(ast::VarPrem {
            id: id("bound", "prem.watsup"),
            plain_typ: bool_typ(),
        }))),
        iter: ast::Iter::List,
    }));
    let rule = p4spec_rust::phrase! { node: ast::RuleKind {
        id_rel: id("relation", "rule.watsup"),
        id_rule: id("", "rule.watsup"),
        exp: exp_upd.clone(),
        prems: vec![
            iteration,
            prem(ast::PremKind::If(ast::IfPrem {
                exp: id_exp("guard"),
            })),
        ],
    }, span: span("rule.watsup") };
    let def_func = definition(ast::DefKind::FuncDef(ast::FuncDef {
        id: id("function", "def.watsup"),
        tparams: vec![p4spec_rust::phrase! {
            node: "T".to_owned(),
            span: span("def.watsup"),
        }],
        args: vec![p4spec_rust::phrase! {
            node: ast::ArgKind::Exp(Box::new(id_exp("argument"))),
            span: span("def.watsup"),
        }],
        exp: id_exp("body"),
        prems: vec![prem(ast::PremKind::Debug(ast::DebugPrem { exp: id_exp("debug") }))],
    }));

    assert_eq!(
        definition(ast::DefKind::RuleGroup(ast::RuleGroupDef {
            relid: id("relation", "def.watsup"),
            groupid: id("group", "def.watsup"),
            rules: vec![rule],
        }))
        .free(),
        ids(&["argument", "bound", "field", "guard", "high", "index", "low"])
    );
    assert_eq!(def_func.free(), ids(&["argument", "body", "debug"]));
}
