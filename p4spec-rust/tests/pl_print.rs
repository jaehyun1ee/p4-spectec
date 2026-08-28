use p4spec_rust::{
    lang::common::source::{Position, Span, Spanned},
    lang::{
        common::notation::mixfix::Mixfix,
        hints::{alter, input::InputHint},
        il, pl,
        traits::print::Print,
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

fn text(value: &str) -> pl::ast::Exp {
    pl::ast::exp(
        pl::ast::ExpKind::Text(value.to_owned()),
        il::ast::TypKind::Text,
        span("text"),
    )
}

fn signature() -> pl::ast::RelSignature {
    pl::ast::RelSignature {
        not_typ: Spanned::new(Mixfix::Arg(typ()), span("signature")),
        input_hint: InputHint::new(vec![0]),
    }
}

fn group_instr(
    kind: pl::ast::InstrKind<pl::ast::InstrGroup>,
) -> pl::ast::Instr<pl::ast::InstrGroup> {
    pl::ast::instr(kind, 1, None, span("group-instruction"))
}

fn dispatch_instr(
    kind: pl::ast::InstrKind<pl::ast::InstrDispatch>,
) -> pl::ast::Instr<pl::ast::InstrDispatch> {
    pl::ast::instr(kind, 1, None, span("dispatch-instruction"))
}

#[test]
fn group_printer_escapes_text_and_omits_annotations_and_fallthrough() {
    let mut first = group_instr(pl::ast::InstrKind::Tier(pl::ast::TierInstr {
        tier: pl::ast::InstrGroup::Return(pl::ast::ReturnGroupInstr {
            exp: text("line\n\"\\"),
        }),
    }));
    first.node.node.note.fallthrough = Some(pl::ast::Fallthrough::FallNext);
    first.hints.prose = Some(alter::AlterationHint::Text("first prose".to_owned()));

    let mut second = first.clone();
    second.node.node.note.iid = 99;
    second.node.node.note.fallthrough = Some(pl::ast::Fallthrough::FallFail);
    second.node.span = span("other-source");
    second.hints.prose = Some(alter::AlterationHint::Text("other prose".to_owned()));

    assert_eq!(Print::render(&vec![first]), "1. Return \"line\\n\\\"\\\\\"");
    assert_eq!(
        Print::render(&vec![second]),
        "1. Return \"line\\n\\\"\\\\\""
    );
}

#[test]
fn shared_control_flow_renders_group_tier_at_nested_level() {
    let branch = group_instr(pl::ast::InstrKind::If(pl::ast::IfInstr {
        exp: variable("condition"),
        iter_exps: Vec::new(),
        block: vec![group_instr(pl::ast::InstrKind::Tier(pl::ast::TierInstr {
            tier: pl::ast::InstrGroup::Return(pl::ast::ReturnGroupInstr {
                exp: variable("value"),
            }),
        }))],
        dangle: true,
    }));
    let mut branch = branch;
    branch.node.node.note.iid = 42;

    assert_eq!(
        Print::render(&vec![branch]),
        concat!(
            "1. If (condition), then\n\n",
            "  1. Return value\n\n",
            "1. Else Dangling#42",
        )
    );
}

#[test]
fn group_and_dispatch_backtracking_preserve_arm_order() {
    let backtrack = group_instr(pl::ast::InstrKind::Tier(pl::ast::TierInstr {
        tier: pl::ast::InstrGroup::Backtrack(pl::ast::BacktrackGroupInstr {
            blocks: vec![
                vec![group_instr(pl::ast::InstrKind::Tier(pl::ast::TierInstr {
                    tier: pl::ast::InstrGroup::Return(pl::ast::ReturnGroupInstr {
                        exp: variable("a"),
                    }),
                }))],
                vec![group_instr(pl::ast::InstrKind::Tier(pl::ast::TierInstr {
                    tier: pl::ast::InstrGroup::Return(pl::ast::ReturnGroupInstr {
                        exp: variable("b"),
                    }),
                }))],
            ],
        }),
    }));
    assert_eq!(
        Print::render(&vec![backtrack]),
        concat!(
            "1. Block (2 arms)\n\n",
            "Arm 1:\n\n  1. Return a\n\n",
            "Arm 2:\n\n  1. Return b",
        )
    );

    let group = |name: &str| {
        vec![dispatch_instr(pl::ast::InstrKind::Tier(
            pl::ast::TierInstr {
                tier: pl::ast::InstrDispatch::Group(pl::ast::GroupDispatchInstr {
                    id_rel: id("relation"),
                    id_group: id(name),
                    rel_signature: signature(),
                    exps_input: vec![variable(name)],
                    block: vec![group_instr(pl::ast::InstrKind::Tier(pl::ast::TierInstr {
                        tier: pl::ast::InstrGroup::Result(pl::ast::ResultGroupInstr {
                            rel_signature: signature(),
                            exps_output: Vec::new(),
                        }),
                    }))],
                }),
            },
        ))]
    };
    let route = dispatch_instr(pl::ast::InstrKind::Tier(pl::ast::TierInstr {
        tier: pl::ast::InstrDispatch::Route(pl::ast::RouteDispatchInstr {
            blocks: vec![group("first"), group("second")],
        }),
    }));
    assert_eq!(
        Print::render(&vec![route]),
        concat!(
            "1. Block (2 arms)\n\n",
            "Arm 1:\n\n  1. Group first: first\n\n",
            "    1. The relation holds\n\n",
            "Arm 2:\n\n  1. Group second: second\n\n",
            "    1. The relation holds",
        )
    );
}
