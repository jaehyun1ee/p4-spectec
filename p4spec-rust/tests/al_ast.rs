use p4spec_rust::{
    domain::{
        mixfix::Mixfix,
        source::{Region, Spanned},
    },
    lang::{al, il},
};

fn span(name: &str) -> Region {
    Region::for_file(name)
}

fn id(name: &str) -> il::ast::Id {
    Spanned::new(name.to_owned(), span(name))
}

fn typ() -> il::ast::Typ {
    Spanned::new(il::ast::TypKind::BoolT, span("type"))
}

fn not_typ() -> il::ast::NotTyp {
    Spanned::new(Mixfix::Seq(Vec::new()), span("notation"))
}

fn exp() -> il::ast::Exp {
    il::ast::Exp::new(
        il::ast::ExpKind::BoolE(true),
        il::ast::TypKind::BoolT,
        span("exp"),
    )
}

fn param() -> il::ast::Param {
    Spanned::new(il::ast::ParamKind::ExpP(typ()), span("param"))
}

fn arg() -> il::ast::Arg {
    Spanned::new(il::ast::ArgKind::ExpA(exp()), span("arg"))
}

fn clause() -> il::ast::Clause {
    Spanned::new((vec![arg()], exp(), Vec::new()), span("clause"))
}

#[test]
fn al_reuses_il_shared_model_types() {
    let id_il = id("shared");
    let typ_il = typ();
    let param_il = param();
    let clause_il = clause();

    let id_al: al::ast::Id = id_il.clone();
    let typ_al: al::ast::Typ = typ_il.clone();
    let param_al: al::ast::Param = param_il.clone();
    let clause_al: al::ast::Clause = clause_il.clone();

    assert_eq!(id_al, id_il);
    assert_eq!(typ_al, typ_il);
    assert_eq!(param_al, param_il);
    assert_eq!(clause_al, clause_il);
}

#[test]
fn al_constructs_rule_else_and_table_shapes_with_their_spans() {
    let rule_match: al::ast::RuleMatch = (vec![exp()], vec![exp()], Vec::new());
    let rule_path: al::ast::RulePath = (id("path"), Vec::new(), vec![exp()]);
    let rule_group: al::ast::RuleGroup = Spanned::new(
        (id("group"), rule_match.clone(), vec![rule_path.clone()]),
        span("rule-group"),
    );
    let else_group: al::ast::ElseGroup =
        Spanned::new((id("else"), rule_match, rule_path), span("else-group"));
    let table_row: al::ast::TableRow = Spanned::new(
        (vec![exp()], vec![arg()], exp(), Vec::new()),
        span("table-row"),
    );

    assert_eq!(rule_group.span, span("rule-group"));
    assert_eq!(rule_group.node.1.0.len(), 1);
    assert_eq!(rule_group.node.2.len(), 1);
    assert_eq!(else_group.span, span("else-group"));
    assert_eq!(else_group.node.1.1.len(), 1);
    assert_eq!(table_row.span, span("table-row"));
    assert_eq!(table_row.node.0.len(), 1);
    assert_eq!(table_row.node.1.len(), 1);
    assert_eq!(table_row.node.3.len(), 0);
}

#[test]
fn al_constructs_every_definition_variant() {
    let rule_match: al::ast::RuleMatch = (Vec::new(), Vec::new(), Vec::new());
    let rule_path: al::ast::RulePath = (id("path"), Vec::new(), Vec::new());
    let rule_group: al::ast::RuleGroup = Spanned::new(
        (id("group"), rule_match.clone(), vec![rule_path.clone()]),
        span("rule-group"),
    );
    let else_group: al::ast::ElseGroup =
        Spanned::new((id("else"), rule_match, rule_path), span("else-group"));
    let table_row: al::ast::TableRow = Spanned::new(
        (vec![exp()], vec![arg()], exp(), Vec::new()),
        span("table-row"),
    );
    let def_type = Spanned::new(il::ast::DefTypKind::PlainT(typ()), span("defined-type"));
    let definitions: al::ast::Spec = vec![
        Spanned::new(
            al::ast::DefKind::ExternTypD(id("extern-type"), Vec::new()),
            span("def"),
        ),
        Spanned::new(
            al::ast::DefKind::TypD(id("type"), Vec::new(), def_type, Vec::new()),
            span("def"),
        ),
        Spanned::new(
            al::ast::DefKind::VarD(id("var"), typ(), Vec::new()),
            span("def"),
        ),
        Spanned::new(
            al::ast::DefKind::ExternRelD(id("extern-rel"), not_typ(), vec![0], Vec::new()),
            span("def"),
        ),
        Spanned::new(
            al::ast::DefKind::RelD(
                id("rel"),
                not_typ(),
                vec![0],
                vec![rule_group],
                Some(else_group),
                Vec::new(),
            ),
            span("def"),
        ),
        Spanned::new(
            al::ast::DefKind::ExternDecD(
                id("extern-dec"),
                Vec::new(),
                vec![param()],
                typ(),
                Vec::new(),
            ),
            span("def"),
        ),
        Spanned::new(
            al::ast::DefKind::BuiltinDecD(
                id("builtin-dec"),
                Vec::new(),
                vec![param()],
                typ(),
                Vec::new(),
            ),
            span("def"),
        ),
        Spanned::new(
            al::ast::DefKind::TableDecD(
                id("table-dec"),
                vec![param()],
                typ(),
                vec![table_row],
                Vec::new(),
            ),
            span("def"),
        ),
        Spanned::new(
            al::ast::DefKind::FuncDecD(
                id("func-dec"),
                Vec::new(),
                vec![param()],
                typ(),
                vec![clause()],
                Some(clause()),
                Vec::new(),
            ),
            span("def"),
        ),
    ];

    let tags = definitions
        .iter()
        .map(|definition| match definition.node {
            al::ast::DefKind::ExternTypD(..) => "ExternTypD",
            al::ast::DefKind::TypD(..) => "TypD",
            al::ast::DefKind::VarD(..) => "VarD",
            al::ast::DefKind::ExternRelD(..) => "ExternRelD",
            al::ast::DefKind::RelD(..) => "RelD",
            al::ast::DefKind::ExternDecD(..) => "ExternDecD",
            al::ast::DefKind::BuiltinDecD(..) => "BuiltinDecD",
            al::ast::DefKind::TableDecD(..) => "TableDecD",
            al::ast::DefKind::FuncDecD(..) => "FuncDecD",
        })
        .collect::<Vec<_>>();

    assert_eq!(
        tags,
        [
            "ExternTypD",
            "TypD",
            "VarD",
            "ExternRelD",
            "RelD",
            "ExternDecD",
            "BuiltinDecD",
            "TableDecD",
            "FuncDecD",
        ]
    );
    assert!(
        definitions
            .iter()
            .all(|definition| definition.span == span("def"))
    );
}
