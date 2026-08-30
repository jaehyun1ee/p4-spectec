use p4spec_rust::{
    lang::common::source::{Position, Span},
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
    p4spec_rust::phrase! {
        node: name.to_owned(),
        span: span(name),
    }
}

fn typ() -> il::ast::Typ {
    p4spec_rust::phrase! {
        node: il::ast::TypKind::Bool,
        span: span("type"),
    }
}

fn variable(name: &str) -> pl::ast::Exp {
    pl::annot::Annotated {
        node: p4spec_rust::note_phrase! {
            node: pl::ast::ExpKind::Var(id(name)),
            note: il::ast::TypKind::Bool,
            span: span(name),
        },
        hints: pl::annot::Hints::default(),
    }
}

fn text(value: &str) -> pl::ast::Exp {
    pl::annot::Annotated {
        node: p4spec_rust::note_phrase! {
            node: pl::ast::ExpKind::Text(value.to_owned()),
            note: il::ast::TypKind::Text,
            span: span("text"),
        },
        hints: pl::annot::Hints::default(),
    }
}

fn signature() -> pl::ast::RelSignature {
    pl::ast::RelSignature {
        not_typ: p4spec_rust::phrase! {
            node: Mixfix::Arg(typ()),
            span: span("signature"),
        },
        input_hint: InputHint::new(vec![0]),
    }
}

fn group_instr(
    kind: pl::ast::InstrKind<pl::ast::InstrGroup>,
) -> pl::ast::Instr<pl::ast::InstrGroup> {
    pl::annot::Annotated {
        node: p4spec_rust::note_phrase! { node: kind, note: pl::ast::InstrNote {
            iid: 1,
            fallthrough: None,
        }, span: span("group-instruction") },
        hints: pl::annot::Hints::default(),
    }
}

fn dispatch_instr(
    kind: pl::ast::InstrKind<pl::ast::InstrDispatch>,
) -> pl::ast::Instr<pl::ast::InstrDispatch> {
    pl::annot::Annotated {
        node: p4spec_rust::note_phrase! { node: kind, note: pl::ast::InstrNote {
            iid: 1,
            fallthrough: None,
        }, span: span("dispatch-instruction") },
        hints: pl::annot::Hints::default(),
    }
}

#[test]
fn group_printer_escapes_text_and_omits_annotations_and_fallthrough() {
    let mut first = group_instr(pl::ast::InstrKind::Tier(pl::ast::TierInstr {
        tier: pl::ast::InstrGroup::Return(pl::ast::ReturnGroupInstr {
            exp: text("line\n\"\\"),
        }),
    }));
    first.node.note.fallthrough = Some(pl::ast::Fallthrough::FallNext);
    first.hints.prose = Some(alter::AlterationHint::Text("first prose".to_owned()));

    let mut second = first.clone();
    second.node.note.iid = 99;
    second.node.note.fallthrough = Some(pl::ast::Fallthrough::FallFail);
    second.node.span = span("other-source");
    second.hints.prose = Some(alter::AlterationHint::Text("other prose".to_owned()));

    assert_eq!(
        Print::to_string(&vec![first]),
        "1. Return \"line\\n\\\"\\\\\""
    );
    assert_eq!(
        Print::to_string(&vec![second]),
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
    branch.node.note.iid = 42;

    assert_eq!(
        Print::to_string(&vec![branch]),
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
        Print::to_string(&vec![backtrack]),
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
        Print::to_string(&vec![route]),
        concat!(
            "1. Block (2 arms)\n\n",
            "Arm 1:\n\n  1. Group first: first\n\n",
            "    1. The relation holds\n\n",
            "Arm 2:\n\n  1. Group second: second\n\n",
            "    1. The relation holds",
        )
    );
}
