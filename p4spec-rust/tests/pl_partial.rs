use p4spec_rust::{
    domain::{
        mixfix::Mixfix,
        source::{Region, Spanned},
    },
    lang::{il, pl},
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

fn exp(kind: pl::ast::ExpKind) -> pl::ast::Exp {
    pl::ast::ExpNode::new(kind, il::ast::TypKind::BoolT, span("expression"))
}

fn variable(name: &str) -> pl::ast::Exp {
    exp(pl::ast::ExpKind::VarE(id(name)))
}

fn call(name: &str) -> pl::ast::Exp {
    exp(pl::ast::ExpKind::CallE(id(name), Vec::new(), Vec::new()))
}

fn group_instr(
    kind: pl::ast::InstrKind<pl::ast::InstrGroup>,
) -> pl::ast::Instr<pl::ast::InstrGroup> {
    pl::ast::InstrNode::new(kind, 0, None, span("instruction"))
}

#[test]
fn expression_and_path_classification_finds_recursive_calls() {
    let nested = exp(pl::ast::ExpKind::TupleE(vec![exp(pl::ast::ExpKind::UpdE(
        Box::new(variable("base")),
        Box::new(pl::ast::Path::new(
            pl::ast::PathKind::IdxP(
                Box::new(pl::ast::Path::new(
                    pl::ast::PathKind::RootP,
                    il::ast::TypKind::BoolT,
                    span("root"),
                )),
                Box::new(call("index")),
            ),
            il::ast::TypKind::BoolT,
            span("path"),
        )),
        Box::new(variable("field")),
    ))]));

    assert!(pl::partial::is_partial_exp(&nested));
    assert!(!pl::partial::is_partial_exp(&variable("plain")));
}

#[test]
fn guard_classification_preserves_immediate_failure_rules() {
    assert!(pl::partial::is_partial_guard(&pl::ast::Guard::CmpG(
        il::ast::CmpOp::EqOp,
        il::ast::OpTyp::BoolT,
        call("comparison"),
    )));
    assert!(pl::partial::is_partial_guard(
        &pl::ast::Guard::CheckLetSubG(typ(), Box::new(il::ast::Subcheck::SkipSC), call("binding"),)
    ));
    assert!(!pl::partial::is_partial_guard(&pl::ast::Guard::MemG(call(
        "membership"
    ))));
    assert!(!pl::partial::is_partial_guard(&pl::ast::Guard::SubG(
        typ(),
        Box::new(il::ast::Subcheck::SkipSC),
    )));
}

#[test]
fn shared_instruction_classification_ignores_nested_block_execution() {
    let nested_call = group_instr(pl::ast::InstrKind::TierI(pl::ast::InstrGroup::ReturnI(
        call("nested"),
    )));
    let branch = group_instr(pl::ast::InstrKind::IfI(
        variable("condition"),
        Vec::new(),
        vec![nested_call],
        false,
    ));
    let hold = group_instr(pl::ast::InstrKind::HoldI(
        id("relation"),
        Mixfix::Arg(variable("argument")),
        Vec::new(),
        pl::ast::HoldCase::HoldH(Vec::new(), false),
    ));

    assert!(!pl::partial::is_partial_instr(
        pl::partial::is_partial_instr_group,
        &branch,
    ));
    assert!(pl::partial::is_partial_instr(
        pl::partial::is_partial_instr_group,
        &hold,
    ));
}

#[test]
fn tier_classification_handles_rules_results_backtracking_and_dispatch() {
    assert!(pl::partial::is_partial_instr_group(
        &pl::ast::InstrGroup::RuleI(
            id("rule"),
            Mixfix::Arg(call("argument")),
            vec![0],
            Vec::new(),
        )
    ));
    assert!(pl::partial::is_partial_instr_group(
        &pl::ast::InstrGroup::ResultI(
            (
                Spanned::new(Mixfix::Arg(typ()), span("signature")),
                Vec::new()
            ),
            vec![call("result")],
        )
    ));
    assert!(!pl::partial::is_partial_instr_group(
        &pl::ast::InstrGroup::BacktrackI(vec![vec![group_instr(pl::ast::InstrKind::TierI(
            pl::ast::InstrGroup::ReturnI(call("arm"))
        ),)]])
    ));
    assert!(!pl::partial::is_partial_instr_dispatch(
        &pl::ast::InstrDispatch::RouteI(Vec::new())
    ));
}
