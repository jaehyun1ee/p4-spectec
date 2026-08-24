use p4spec_rust::{
    domain::{
        mixfix::Mixfix,
        source::{Region, Spanned},
    },
    lang::{
        hints::{alter, input::InputHint},
        il, pl,
    },
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

fn variable(name: &str) -> pl::ast::Exp {
    pl::ast::ExpNode::new(
        pl::ast::ExpKind::VarE(id(name)),
        il::ast::TypKind::BoolT,
        span(name),
    )
}

fn group(name: &str) -> pl::ast::Instr<pl::ast::InstrDispatch> {
    let mut instruction = pl::ast::InstrNode::new(
        pl::ast::InstrKind::TierI(pl::ast::InstrDispatch::GroupI {
            group_id: id(name),
            relation_id: id("relation"),
            signature: pl::ast::RelSignature {
                notation: Spanned::new(Mixfix::Arg(typ()), span("signature")),
                input_hint: InputHint::new(vec![0]),
            },
            inputs: vec![variable(name)],
            block: Vec::new(),
        }),
        0,
        None,
        span(name),
    );
    instruction.hints.prose = Some(alter::AlterationHint::TextH(format!("hint-{name}")));
    instruction
}

fn control(
    kind: pl::ast::InstrKind<pl::ast::InstrDispatch>,
) -> pl::ast::Instr<pl::ast::InstrDispatch> {
    pl::ast::InstrNode::new(kind, 0, None, span("control"))
}

#[test]
fn group_collection_preserves_depth_first_branch_order_and_hints() {
    let block = vec![
        group("a"),
        control(pl::ast::InstrKind::IfI(
            variable("if"),
            Vec::new(),
            vec![group("b")],
            false,
        )),
        control(pl::ast::InstrKind::HoldI(
            id("hold"),
            Mixfix::Arg(variable("held")),
            Vec::new(),
            pl::ast::HoldCase::BothH(vec![group("c")], vec![group("d")]),
        )),
        control(pl::ast::InstrKind::CaseI(
            variable("case"),
            vec![
                pl::ast::Case {
                    guard: pl::ast::Guard::BoolG(true),
                    block: vec![group("e")],
                },
                pl::ast::Case {
                    guard: pl::ast::Guard::BoolG(false),
                    block: vec![group("f")],
                },
            ],
            false,
        )),
        control(pl::ast::InstrKind::CheckLetSubI(
            typ(),
            Box::new(il::ast::Subcheck::SkipSC),
            variable("left"),
            variable("right"),
            vec![group("g")],
        )),
        control(pl::ast::InstrKind::TierI(pl::ast::InstrDispatch::RouteI(
            vec![vec![group("h")], vec![group("i")]],
        ))),
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
        Some(alter::AlterationHint::TextH("hint-a".to_owned()))
    );
    assert_eq!(groups[0].id_rel.node, "relation");
    assert_eq!(groups[0].exps[0].node.span, span("a"));
}
