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

fn text(value: &str) -> pl::ast::Exp {
    pl::ast::ExpNode::new(
        pl::ast::ExpKind::TextE(value.to_owned()),
        il::ast::TypKind::TextT,
        span("text"),
    )
}

fn signature() -> pl::ast::RelSignature {
    (
        Spanned::new(Mixfix::Arg(typ()), span("signature")),
        InputHint::new(vec![0]),
    )
}

fn group_instr(
    kind: pl::ast::InstrKind<pl::ast::InstrGroup>,
) -> pl::ast::Instr<pl::ast::InstrGroup> {
    pl::ast::InstrNode::new(kind, 1, None, span("group-instruction"))
}

fn dispatch_instr(
    kind: pl::ast::InstrKind<pl::ast::InstrDispatch>,
) -> pl::ast::Instr<pl::ast::InstrDispatch> {
    pl::ast::InstrNode::new(kind, 1, None, span("dispatch-instruction"))
}

#[test]
fn group_printer_escapes_text_and_omits_annotations_and_fallthrough() {
    let mut first = group_instr(pl::ast::InstrKind::TierI(pl::ast::InstrGroup::ReturnI(
        text("line\n\"\\"),
    )));
    first.node.fallthrough = Some(pl::ast::Fallthrough::FallNext);
    first.hints.prose = Some(alter::AlterationHint::TextH("first prose".to_owned()));

    let mut second = first.clone();
    second.node.iid = 99;
    second.node.fallthrough = Some(pl::ast::Fallthrough::FallFail);
    second.node.span = span("other-source");
    second.hints.prose = Some(alter::AlterationHint::TextH("other prose".to_owned()));

    assert_eq!(
        pl::print::string_of_block_group(&vec![first]),
        "1. Return \"line\\n\\\"\\\\\""
    );
    assert_eq!(
        pl::print::string_of_block_group(&vec![second]),
        "1. Return \"line\\n\\\"\\\\\""
    );
}

#[test]
fn shared_control_flow_renders_group_tier_at_nested_level() {
    let branch = group_instr(pl::ast::InstrKind::IfI(
        variable("condition"),
        Vec::new(),
        vec![group_instr(pl::ast::InstrKind::TierI(
            pl::ast::InstrGroup::ReturnI(variable("value")),
        ))],
        true,
    ));
    let mut branch = branch;
    branch.node.iid = 42;

    assert_eq!(
        pl::print::string_of_block_group(&vec![branch]),
        concat!(
            "1. If (condition), then\n\n",
            "  1. Return value\n\n",
            "1. Else Dangling#42",
        )
    );
}

#[test]
fn group_and_dispatch_backtracking_preserve_arm_order() {
    let backtrack = group_instr(pl::ast::InstrKind::TierI(pl::ast::InstrGroup::BacktrackI(
        vec![
            vec![group_instr(pl::ast::InstrKind::TierI(
                pl::ast::InstrGroup::ReturnI(variable("a")),
            ))],
            vec![group_instr(pl::ast::InstrKind::TierI(
                pl::ast::InstrGroup::ReturnI(variable("b")),
            ))],
        ],
    )));
    assert_eq!(
        pl::print::string_of_block_group(&vec![backtrack]),
        concat!(
            "1. Block (2 arms)\n\n",
            "Arm 1:\n\n  1. Return a\n\n",
            "Arm 2:\n\n  1. Return b",
        )
    );

    let group = |name: &str| {
        vec![dispatch_instr(pl::ast::InstrKind::TierI(
            pl::ast::InstrDispatch::GroupI(
                id(name),
                id("relation"),
                signature(),
                vec![variable(name)],
                vec![group_instr(pl::ast::InstrKind::TierI(
                    pl::ast::InstrGroup::ResultI(signature(), Vec::new()),
                ))],
            ),
        ))]
    };
    let route = dispatch_instr(pl::ast::InstrKind::TierI(pl::ast::InstrDispatch::RouteI(
        vec![group("first"), group("second")],
    )));
    assert_eq!(
        pl::print::string_of_block_dispatch(&vec![route]),
        concat!(
            "1. Block (2 arms)\n\n",
            "Arm 1:\n\n  1. Group first: first\n\n",
            "    1. The relation holds\n\n",
            "Arm 2:\n\n  1. Group second: second\n\n",
            "    1. The relation holds",
        )
    );
}
