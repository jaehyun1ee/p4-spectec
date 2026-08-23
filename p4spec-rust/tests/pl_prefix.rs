use p4spec_rust::{
    domain::{
        atom::Atom,
        mixfix::Mixfix,
        source::{HasSpan, Region, Spanned, phrase_list_region},
    },
    lang::{
        hints::{alter, fields},
        il, pl, sl,
    },
};

fn span(name: &str) -> Region {
    Region::for_file(name)
}
fn id(name: &str) -> pl::ast::Id {
    Spanned::new(name.into(), span(name))
}
fn typ() -> pl::ast::Typ {
    Spanned::new(sl::ast::TypKind::BoolT, span("typ"))
}
fn atom(name: &str) -> pl::ast::Atom {
    Spanned::new(Atom::Keyword(name.into()), span(name))
}
fn exp(kind: pl::ast::ExpKind, name: &str) -> pl::ast::Exp {
    pl::ast::ExpNode::new(kind, sl::ast::TypKind::BoolT, span(name))
}
fn variable(name: &str) -> pl::ast::Exp {
    exp(pl::ast::ExpKind::VarE(id(name)), name)
}
fn path(kind: pl::ast::PathKind, name: &str) -> pl::ast::Path {
    pl::ast::Path::new(kind, sl::ast::TypKind::BoolT, span(name))
}
fn tag(exp: &pl::ast::Exp) -> &'static str {
    match &exp.node.kind {
        pl::ast::ExpKind::BoolE(_) => "BoolE",
        pl::ast::ExpKind::NumE(_) => "NumE",
        pl::ast::ExpKind::TextE(_) => "TextE",
        pl::ast::ExpKind::VarE(_) => "VarE",
        pl::ast::ExpKind::UnE(..) => "UnE",
        pl::ast::ExpKind::BinE(..) => "BinE",
        pl::ast::ExpKind::CmpE(..) => "CmpE",
        pl::ast::ExpKind::UpCastE(..) => "UpCastE",
        pl::ast::ExpKind::DownCastE(..) => "DownCastE",
        pl::ast::ExpKind::SubE(..) => "SubE",
        pl::ast::ExpKind::MatchE(..) => "MatchE",
        pl::ast::ExpKind::TupleE(..) => "TupleE",
        pl::ast::ExpKind::CaseE(..) => "CaseE",
        pl::ast::ExpKind::StrE(..) => "StrE",
        pl::ast::ExpKind::OptE(..) => "OptE",
        pl::ast::ExpKind::ListE(..) => "ListE",
        pl::ast::ExpKind::ConsE(..) => "ConsE",
        pl::ast::ExpKind::CatE(..) => "CatE",
        pl::ast::ExpKind::MemE(..) => "MemE",
        pl::ast::ExpKind::LenE(..) => "LenE",
        pl::ast::ExpKind::DotE(..) => "DotE",
        pl::ast::ExpKind::IdxE(..) => "IdxE",
        pl::ast::ExpKind::SliceE(..) => "SliceE",
        pl::ast::ExpKind::UpdE(..) => "UpdE",
        pl::ast::ExpKind::CallE(..) => "CallE",
        pl::ast::ExpKind::IterE(..) => "IterE",
    }
}

#[test]
fn pl_prefix_constructs_all_expression_and_path_variants() {
    let root = path(pl::ast::PathKind::RootP, "root");
    let indexed = path(
        pl::ast::PathKind::IdxP(Box::new(root.clone()), Box::new(variable("index"))),
        "indexed",
    );
    let sliced = path(
        pl::ast::PathKind::SliceP(
            Box::new(indexed.clone()),
            Box::new(variable("low")),
            Box::new(variable("high")),
        ),
        "sliced",
    );
    let dotted = path(
        pl::ast::PathKind::DotP(Box::new(sliced.clone()), atom("field")),
        "dotted",
    );
    let recursive_mixfix = Mixfix::Brack(
        atom("left"),
        Box::new(Mixfix::Infix(
            Box::new(Mixfix::Arg(variable("case_left"))),
            atom("op"),
            Box::new(Mixfix::Arg(variable("case_right"))),
        )),
        atom("right"),
    );
    let iterexp = (
        sl::ast::Iter::List,
        vec![(id("item"), typ(), vec![sl::ast::Iter::Opt])],
    );
    let expressions = vec![
        exp(pl::ast::ExpKind::BoolE(true), "bool"),
        exp(pl::ast::ExpKind::NumE(il::ast::Num::Nat(1.into())), "num"),
        exp(pl::ast::ExpKind::TextE("text".into()), "text"),
        variable("var"),
        exp(
            pl::ast::ExpKind::UnE(
                sl::ast::UnOp::NotOp,
                sl::ast::OpTyp::BoolT,
                Box::new(variable("un")),
            ),
            "un",
        ),
        exp(
            pl::ast::ExpKind::BinE(
                sl::ast::BinOp::AddOp,
                sl::ast::OpTyp::NatT,
                Box::new(variable("left")),
                Box::new(variable("right")),
            ),
            "bin",
        ),
        exp(
            pl::ast::ExpKind::CmpE(
                sl::ast::CmpOp::EqOp,
                sl::ast::OpTyp::BoolT,
                Box::new(variable("left")),
                Box::new(variable("right")),
            ),
            "cmp",
        ),
        exp(
            pl::ast::ExpKind::UpCastE(typ(), Box::new(variable("cast"))),
            "upcast",
        ),
        exp(
            pl::ast::ExpKind::DownCastE(typ(), Box::new(variable("cast"))),
            "downcast",
        ),
        exp(
            pl::ast::ExpKind::SubE(
                Box::new(variable("sub")),
                typ(),
                Box::new(il::ast::Subcheck::SkipSC),
            ),
            "sub",
        ),
        exp(
            pl::ast::ExpKind::MatchE(
                Box::new(variable("match")),
                sl::ast::Pattern::OptP(il::ast::OptPattern::Some),
            ),
            "match",
        ),
        exp(pl::ast::ExpKind::TupleE(vec![variable("tuple")]), "tuple"),
        exp(pl::ast::ExpKind::CaseE(Box::new(recursive_mixfix)), "case"),
        exp(
            pl::ast::ExpKind::StrE(vec![(atom("record"), variable("field"))]),
            "struct",
        ),
        exp(
            pl::ast::ExpKind::OptE(Some(Box::new(variable("option")))),
            "option",
        ),
        exp(pl::ast::ExpKind::ListE(vec![variable("list")]), "list"),
        exp(
            pl::ast::ExpKind::ConsE(Box::new(variable("head")), Box::new(variable("tail"))),
            "cons",
        ),
        exp(
            pl::ast::ExpKind::CatE(Box::new(variable("left")), Box::new(variable("right"))),
            "cat",
        ),
        exp(
            pl::ast::ExpKind::MemE(Box::new(variable("member")), Box::new(variable("set"))),
            "mem",
        ),
        exp(pl::ast::ExpKind::LenE(Box::new(variable("length"))), "len"),
        exp(
            pl::ast::ExpKind::DotE(Box::new(variable("dot")), atom("field")),
            "dot",
        ),
        exp(
            pl::ast::ExpKind::IdxE(Box::new(variable("base")), Box::new(variable("index"))),
            "idx",
        ),
        exp(
            pl::ast::ExpKind::SliceE(
                Box::new(variable("base")),
                Box::new(variable("low")),
                Box::new(variable("high")),
            ),
            "slice",
        ),
        exp(
            pl::ast::ExpKind::UpdE(
                Box::new(variable("base")),
                Box::new(dotted.clone()),
                Box::new(variable("value")),
            ),
            "update",
        ),
        exp(
            pl::ast::ExpKind::CallE(
                id("call"),
                vec![typ()],
                vec![Spanned::new(
                    pl::ast::ArgKind::ExpA(variable("argument")),
                    span("arg"),
                )],
            ),
            "call",
        ),
        exp(
            pl::ast::ExpKind::IterE(Box::new(variable("iterated")), iterexp),
            "iter",
        ),
    ];
    assert_eq!(
        expressions.iter().map(tag).collect::<Vec<_>>(),
        vec![
            "BoolE",
            "NumE",
            "TextE",
            "VarE",
            "UnE",
            "BinE",
            "CmpE",
            "UpCastE",
            "DownCastE",
            "SubE",
            "MatchE",
            "TupleE",
            "CaseE",
            "StrE",
            "OptE",
            "ListE",
            "ConsE",
            "CatE",
            "MemE",
            "LenE",
            "DotE",
            "IdxE",
            "SliceE",
            "UpdE",
            "CallE",
            "IterE"
        ]
    );
    assert!(matches!(root.kind, pl::ast::PathKind::RootP));
    assert!(matches!(indexed.kind, pl::ast::PathKind::IdxP(_, _)));
    assert!(matches!(sliced.kind, pl::ast::PathKind::SliceP(_, _, _)));
    assert!(matches!(dotted.kind, pl::ast::PathKind::DotP(_, _)));
    let pl::ast::ExpKind::UpdE(base, update_path, value) = &expressions[23].node.kind else {
        panic!("update shape")
    };
    assert_eq!(base.node.span, span("base"));
    assert_eq!(update_path.span, span("dotted"));
    assert_eq!(value.node.span, span("value"));
    let pl::ast::ExpKind::CaseE(case) = &expressions[12].node.kind else {
        panic!("case shape")
    };
    assert!(
        matches!(case.as_ref(), Mixfix::Brack(_, inner, _) if matches!(inner.as_ref(), Mixfix::Infix(_, _, _)))
    );
}

#[test]
fn pl_prefix_preserves_param_argument_alias_and_annotation_shapes() {
    let child = Spanned::new(
        pl::ast::ParamKind::ExpP(typ(), variable("parameter_exp")),
        span("child"),
    );
    let parameter = Spanned::new(
        pl::ast::ParamKind::DefP(
            id("definition"),
            vec![Spanned::new("T".into(), span("tparam"))],
            vec![child.clone()],
            typ(),
        ),
        span("param"),
    );
    let argument = Spanned::new(
        pl::ast::ArgKind::DefA(id("argument_definition")),
        span("argument"),
    );
    let pl::ast::ParamKind::DefP(definition, tparams, params, result) = &parameter.node else {
        panic!("definition parameter")
    };
    assert_eq!(definition.node, "definition");
    assert_eq!(tparams[0].node, "T");
    assert!(matches!(params[0].node, pl::ast::ParamKind::ExpP(_, _)));
    assert_eq!(result.node, sl::ast::TypKind::BoolT);
    assert!(
        matches!(argument.node, pl::ast::ArgKind::DefA(ref id) if id.node == "argument_definition")
    );
    let nottyp: pl::ast::NotTypKind = Mixfix::Arg(typ());
    let deftyp: pl::ast::DefTypKind = sl::ast::DefTypKind::PlainT(typ());
    let field: pl::ast::TypField = (atom("field"), typ());
    let case: pl::ast::TypCase = (
        Spanned::new(Mixfix::Arg(typ()), span("notation")),
        Spanned::new((id("origin"), vec![]), span("origin")),
        vec![],
    );
    let value: pl::ast::Value = sl::ast::Value::new(
        sl::ast::ValueKind::BoolV(true),
        sl::ast::TypKind::BoolT,
        span("value"),
    );
    assert!(matches!(nottyp, Mixfix::Arg(_)));
    assert!(matches!(deftyp, sl::ast::DefTypKind::PlainT(_)));
    assert_eq!(field.0.node, Atom::Keyword("field".into()));
    assert_eq!(case.1.node.0.node, "origin");
    assert!(matches!(value.kind, sl::ast::ValueKind::BoolV(true)));
    let nested = pl::annot::T {
        node: variable("nested").node,
        hints: pl::annot::Hints {
            prose: Some(alter::T::TextH("prose".into())),
            prose_in: Some(alter::T::HoleH(alter::Hole::Next)),
            prose_out: Some(alter::T::HoleH(alter::Hole::Num(1))),
            prose_true: Some(alter::T::TextH("true".into())),
            prose_false: Some(alter::T::TextH("false".into())),
            prose_fields: Some(fields::T::from(["field hint".into()])),
            prose_input_exps: Some(vec![il::ast::Exp::new(
                il::ast::ExpKind::BoolE(true),
                il::ast::TypKind::BoolT,
                span("input"),
            )]),
            prose_output_exps: Some(vec![il::ast::Exp::new(
                il::ast::ExpKind::BoolE(false),
                il::ast::TypKind::BoolT,
                span("output"),
            )]),
        },
    };
    assert!(
        nested.hints.prose.is_some()
            && nested.hints.prose_in.is_some()
            && nested.hints.prose_out.is_some()
            && nested.hints.prose_true.is_some()
            && nested.hints.prose_false.is_some()
            && nested.hints.prose_fields.is_some()
            && nested.hints.prose_input_exps.is_some()
            && nested.hints.prose_output_exps.is_some()
    );
    assert_eq!(nested.span(), &span("nested"));
    let outer = pl::annot::no_hints(pl::ast::ExpNode {
        kind: pl::ast::ExpKind::UnE(
            sl::ast::UnOp::NotOp,
            sl::ast::OpTyp::BoolT,
            Box::new(nested),
        ),
        ty: sl::ast::TypKind::BoolT,
        span: span("outer"),
    });
    let pl::ast::ExpKind::UnE(_, _, inner) = &outer.node.kind else {
        panic!("nested annotation")
    };
    assert_eq!(outer.span(), &span("outer"));
    assert_eq!(inner.span(), &span("nested"));
    assert_eq!(phrase_list_region(&[outer]), span("outer"));
    assert_eq!(phrase_list_region(&[dotted_path()]), span("path_span"));
}

fn dotted_path() -> pl::ast::Path {
    path(
        pl::ast::PathKind::DotP(
            Box::new(path(pl::ast::PathKind::RootP, "path_root")),
            atom("path_field"),
        ),
        "path_span",
    )
}

#[test]
fn pl_control_and_definition_tiers_construct_every_variant() {
    let group_block: pl::ast::BlockGroup = vec![];
    let dispatch_block: pl::ast::BlockDispatch = vec![];
    let hold_cases = [
        pl::ast::HoldCase::BothH(group_block.clone(), group_block.clone()),
        pl::ast::HoldCase::HoldH(group_block.clone(), true),
        pl::ast::HoldCase::NotHoldH(group_block.clone(), false),
    ];
    let guards = [
        pl::ast::Guard::BoolG(true),
        pl::ast::Guard::CmpG(sl::ast::CmpOp::EqOp, sl::ast::OpTyp::BoolT, variable("cmp")),
        pl::ast::Guard::SubG(typ(), Box::new(il::ast::Subcheck::SkipSC)),
        pl::ast::Guard::MatchG(sl::ast::Pattern::OptP(il::ast::OptPattern::Some)),
        pl::ast::Guard::MemG(variable("member")),
        pl::ast::Guard::CheckLetSubG(typ(), Box::new(il::ast::Subcheck::SkipSC), variable("sub")),
        pl::ast::Guard::CheckLetMatchG(
            sl::ast::Pattern::OptP(il::ast::OptPattern::None),
            variable("matched"),
        ),
    ];
    let control = vec![
        pl::ast::InstrKind::IfI(variable("if"), vec![], group_block.clone(), false),
        pl::ast::InstrKind::HoldI(
            id("rel"),
            Mixfix::Arg(variable("hold")),
            vec![],
            hold_cases[0].clone(),
        ),
        pl::ast::InstrKind::CaseI(
            variable("case"),
            vec![(guards[0].clone(), group_block.clone())],
            false,
        ),
        pl::ast::InstrKind::LetI(variable("left"), variable("right"), vec![]),
        pl::ast::InstrKind::DebugI(variable("debug")),
        pl::ast::InstrKind::DestructI(
            vec![
                (Some("name".into()), variable("value")),
                (None, variable("discard")),
            ],
            variable("source"),
        ),
        pl::ast::InstrKind::CheckLetSubI(
            typ(),
            Box::new(il::ast::Subcheck::SkipSC),
            variable("left"),
            variable("right"),
            group_block.clone(),
        ),
        pl::ast::InstrKind::CheckLetMatchI(
            sl::ast::Pattern::ListP(il::ast::ListPattern::Nil),
            variable("left"),
            variable("right"),
            group_block.clone(),
        ),
        pl::ast::InstrKind::OptionGetI(variable("option"), variable("value"), group_block.clone()),
        pl::ast::InstrKind::TierI(pl::ast::InstrGroup::ReturnI(variable("tier"))),
    ];
    let instruction = pl::ast::InstrNode::new(
        control[0].clone(),
        7,
        Some(pl::ast::Fallthrough::FallNext),
        span("instruction"),
    );
    assert_eq!(instruction.span(), &span("instruction"));
    assert!(matches!(
        instruction.node.fallthrough,
        Some(pl::ast::Fallthrough::FallNext)
    ));
    let pl::ast::InstrKind::IfI(condition, iterexps, block, dangle) = &instruction.node.kind else {
        panic!("if instruction")
    };
    assert_eq!(condition.node.span, span("if"));
    assert!(iterexps.is_empty());
    assert!(block.is_empty());
    assert!(!dangle);
    let nested_dispatch = pl::ast::InstrNode::new(
        pl::ast::InstrKind::TierI(pl::ast::InstrGroup::BacktrackI(vec![group_block.clone()])),
        8,
        Some(pl::ast::Fallthrough::FallGroup(id("next_group"))),
        span("nested_dispatch"),
    );
    let pl::ast::InstrKind::TierI(pl::ast::InstrGroup::BacktrackI(blocks)) =
        &nested_dispatch.node.kind
    else {
        panic!("tier backtrack")
    };
    let [only_block] = blocks.as_slice() else {
        panic!("backtrack blocks")
    };
    assert!(only_block.is_empty());
    assert!(
        matches!(&nested_dispatch.node.fallthrough, Some(pl::ast::Fallthrough::FallGroup(id)) if id.node == "next_group")
    );
    let groups = vec![
        pl::ast::InstrGroup::ResultI(
            (Spanned::new(Mixfix::Arg(typ()), span("signature")), vec![0]),
            vec![variable("result")],
        ),
        pl::ast::InstrGroup::ReturnI(variable("return")),
        pl::ast::InstrGroup::RuleI(id("rule"), Mixfix::Arg(variable("input")), vec![0], vec![]),
        pl::ast::InstrGroup::BacktrackI(vec![group_block.clone()]),
    ];
    let dispatch = vec![
        pl::ast::InstrDispatch::GroupI(
            id("group"),
            id("relation"),
            (Spanned::new(Mixfix::Arg(typ()), span("signature")), vec![0]),
            vec![variable("argument")],
            group_block.clone(),
        ),
        pl::ast::InstrDispatch::RouteI(vec![dispatch_block.clone()]),
    ];
    let [_, _, group_rule, _] = groups.as_slice() else {
        panic!("group tiers")
    };
    let pl::ast::InstrGroup::RuleI(rule_id, rule_notation, inputs, iterinstrs) = group_rule else {
        panic!("group rule")
    };
    assert_eq!(rule_id.node, "rule");
    assert!(matches!(rule_notation, Mixfix::Arg(exp) if exp.node.span == span("input")));
    assert_eq!(inputs, &vec![0]);
    assert!(iterinstrs.is_empty());
    let [dispatch_group, _] = dispatch.as_slice() else {
        panic!("dispatch tiers")
    };
    let pl::ast::InstrDispatch::GroupI(group_id, rel_id, signature, arguments, body) =
        dispatch_group
    else {
        panic!("dispatch group")
    };
    assert_eq!(group_id.node, "group");
    assert_eq!(rel_id.node, "relation");
    assert!(matches!(signature.0.node, Mixfix::Arg(_)));
    assert_eq!(arguments[0].node.span, span("argument"));
    assert!(body.is_empty());
    let definitions = vec![
        pl::ast::DefNode::new(
            pl::ast::DefKind::ExternTypD(id("Syntax")),
            span("extern_type"),
        ),
        pl::ast::DefNode::new(
            pl::ast::DefKind::TypD(
                id("Alias"),
                vec![],
                Spanned::new(sl::ast::DefTypKind::PlainT(typ()), span("deftyp")),
            ),
            span("type"),
        ),
        pl::ast::DefNode::new(pl::ast::DefKind::VarD(id("value"), typ()), span("var")),
        pl::ast::DefNode::new(
            pl::ast::DefKind::ExternRelD((
                id("external"),
                (Spanned::new(Mixfix::Arg(typ()), span("signature")), vec![]),
                vec![variable("argument")],
            )),
            span("extern_rel"),
        ),
        pl::ast::DefNode::new(
            pl::ast::DefKind::RelD((
                id("relation"),
                (Spanned::new(Mixfix::Arg(typ()), span("signature")), vec![]),
                vec![variable("argument")],
                dispatch_block.clone(),
                Some(dispatch_block.clone()),
            )),
            span("rel"),
        ),
        pl::ast::DefNode::new(
            pl::ast::DefKind::ExternDecD((id("extern"), vec![], vec![], typ())),
            span("extern_dec"),
        ),
        pl::ast::DefNode::new(
            pl::ast::DefKind::BuiltinDecD((id("builtin"), vec![], vec![], typ())),
            span("builtin_dec"),
        ),
        pl::ast::DefNode::new(
            pl::ast::DefKind::TableDecD((
                id("table"),
                vec![],
                typ(),
                vec![(
                    vec![variable("key")],
                    variable("result"),
                    group_block.clone(),
                )],
            )),
            span("table_dec"),
        ),
        pl::ast::DefNode::new(
            pl::ast::DefKind::FuncDecD((id("function"), vec![], vec![], typ(), group_block, None)),
            span("function_dec"),
        ),
    ];
    let spec: pl::ast::Spec = definitions;
    let [extern_typ, _, _, _, relation, _, _, table, _] = spec.as_slice() else {
        panic!("definition surface")
    };
    assert_eq!(extern_typ.span(), &span("extern_type"));
    let pl::ast::DefKind::RelD((rel_id, signature, arguments, dispatch, otherwise)) =
        &relation.node.kind
    else {
        panic!("relation definition")
    };
    assert_eq!(rel_id.node, "relation");
    assert!(matches!(signature.0.node, Mixfix::Arg(_)));
    assert_eq!(arguments[0].node.span, span("argument"));
    assert!(dispatch.is_empty());
    assert!(otherwise.as_ref().is_some_and(Vec::is_empty));
    let pl::ast::DefKind::TableDecD((table_id, params, result, rows)) = &table.node.kind else {
        panic!("table definition")
    };
    let [(keys, table_result, table_body)] = rows.as_slice() else {
        panic!("table row")
    };
    assert_eq!(table_id.node, "table");
    assert!(params.is_empty());
    assert_eq!(result.node, sl::ast::TypKind::BoolT);
    assert_eq!(keys[0].node.span, span("key"));
    assert_eq!(table_result.node.span, span("result"));
    assert!(table_body.is_empty());
}
