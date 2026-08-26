use p4spec_rust::{
    lang::common::source::{Position, Span, Spanned},
    lang::{
        common::notation::mixfix::Mixfix,
        hints::{alter, input::InputHint},
        il, pl,
    },
};

fn span(name: &str) -> Span {
    Span::new(Position::new(name, 0, 0), Position::new(name, 0, 0))
}

fn id(name: &str) -> il::ast::Id {
    Spanned::new(name.to_owned(), span(name))
}

fn typ() -> il::ast::Typ {
    Spanned::new(il::ast::TypKind::Bool, span("type"))
}

fn variable(name: &str) -> pl::ast::Exp {
    pl::ast::exp(
        pl::ast::ExpKind::Var(id(name)),
        il::ast::TypKind::Bool,
        span(name),
    )
}

fn group(name: &str) -> pl::ast::Instr<pl::ast::InstrDispatch> {
    let mut instruction = pl::ast::instr(
        pl::ast::InstrKind::Tier(pl::ast::TierInstr {
            tier: pl::ast::InstrDispatch::Group(pl::ast::GroupDispatchInstr {
                id_rel: id("relation"),
                id_group: id(name),
                rel_signature: pl::ast::RelSignature {
                    not_typ: Spanned::new(Mixfix::Arg(typ()), span("signature")),
                    input_hint: InputHint::new(vec![0]),
                },
                exps_input: vec![variable(name)],
                block: Vec::new(),
            }),
        }),
        0,
        None,
        span(name),
    );
    instruction.hints.prose = Some(alter::AlterationHint::Text(format!("hint-{name}")));
    instruction
}

fn control(
    kind: pl::ast::InstrKind<pl::ast::InstrDispatch>,
) -> pl::ast::Instr<pl::ast::InstrDispatch> {
    pl::ast::instr(kind, 0, None, span("control"))
}

#[test]
fn group_collection_preserves_depth_first_branch_order_and_hints() {
    let block = vec![
        group("a"),
        control(pl::ast::InstrKind::If(pl::ast::IfInstr {
            exp: variable("if"),
            iter_exps: Vec::new(),
            block: vec![group("b")],
            dangle: false,
        })),
        control(pl::ast::InstrKind::Hold(pl::ast::HoldInstr {
            id: id("hold"),
            not_exp: Mixfix::Arg(variable("held")),
            iter_exps: Vec::new(),
            hold_case: pl::ast::HoldCase::Both(vec![group("c")], vec![group("d")]),
        })),
        control(pl::ast::InstrKind::Case(pl::ast::CaseInstr {
            exp: variable("case"),
            cases: vec![
                pl::ast::Case {
                    guard: pl::ast::Guard::Bool(true),
                    block: vec![group("e")],
                },
                pl::ast::Case {
                    guard: pl::ast::Guard::Bool(false),
                    block: vec![group("f")],
                },
            ],
            dangle: false,
        })),
        control(pl::ast::InstrKind::CheckLetSub(pl::ast::CheckLetSubInstr {
            typ: typ(),
            subcheck: Box::new(il::ast::Subcheck::Skip),
            exp_l: variable("left"),
            exp_r: variable("right"),
            block: vec![group("g")],
        })),
        control(pl::ast::InstrKind::Tier(pl::ast::TierInstr {
            tier: pl::ast::InstrDispatch::Route(pl::ast::RouteDispatchInstr {
                blocks: vec![vec![group("h")], vec![group("i")]],
            }),
        })),
    ];

    let groups: Vec<pl::group::RuleGroup> = pl::group::collect_groups(&block);
    assert_eq!(
        groups
            .iter()
            .map(|group| group.id_rulegroup.node.as_str())
            .collect::<Vec<_>>(),
        vec!["a", "b", "c", "d", "e", "f", "g", "h", "i"]
    );
    assert_eq!(
        groups[0].hints.prose,
        Some(alter::AlterationHint::Text("hint-a".to_owned()))
    );
    assert_eq!(groups[0].id_rel.node, "relation");
    assert_eq!(groups[0].exps[0].node.span, span("a"));
}
