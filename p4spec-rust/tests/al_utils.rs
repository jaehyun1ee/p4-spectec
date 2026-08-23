use std::collections::BTreeSet;

use p4spec_rust::{
    domain::{
        atom::Atom,
        mixfix::Mixfix,
        source::{Region, Spanned},
    },
    lang::{al, il, xl::num},
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

fn atom() -> il::ast::Atom {
    Spanned::new(Atom::Keyword("A".to_owned()), span("atom"))
}

fn variable(name: &str) -> il::ast::Exp {
    il::ast::Exp::new(
        il::ast::ExpKind::VarE(id(name)),
        il::ast::TypKind::BoolT,
        span(name),
    )
}

fn expr(kind: il::ast::ExpKind) -> il::ast::Exp {
    il::ast::Exp::new(kind, il::ast::TypKind::BoolT, span("expression"))
}

fn not_exp(name: &str) -> il::ast::NotExp {
    Mixfix::Arg(variable(name))
}

fn arg_exp(name: &str) -> il::ast::Arg {
    Spanned::new(il::ast::ArgKind::ExpA(variable(name)), span("arg"))
}

fn path_with(name: &str) -> il::ast::Path {
    il::ast::Path::new(
        il::ast::PathKind::IdxP(
            Box::new(il::ast::Path::new(
                il::ast::PathKind::RootP,
                il::ast::TypKind::BoolT,
                span("root"),
            )),
            Box::new(variable(name)),
        ),
        il::ast::TypKind::BoolT,
        span("path"),
    )
}

fn ids(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn al_equality_delegates_to_span_and_subcheck_insensitive_il_semantics() {
    let left = il::ast::Exp::new(
        il::ast::ExpKind::SubE(
            Box::new(variable("x")),
            typ(),
            Box::new(il::ast::Subcheck::SkipSC),
        ),
        il::ast::TypKind::BoolT,
        span("left"),
    );
    let right = il::ast::Exp::new(
        il::ast::ExpKind::SubE(
            Box::new(variable("x")),
            typ(),
            Box::new(il::ast::Subcheck::RecurseSC(typ())),
        ),
        il::ast::TypKind::TextT,
        span("right"),
    );

    assert!(al::eq::eq_exp(&left, &right));
    assert!(al::eq::eq_id(&id("name"), &id("name")));
    assert!(al::eq::eq_arg(&arg_exp("x"), &arg_exp("x")));
    assert!(al::eq::eq_prem(
        &Spanned::new(il::ast::PremKind::IfPr(variable("x")), span("prem")),
        &Spanned::new(il::ast::PremKind::IfPr(variable("x")), span("other-prem")),
    ));
    let atom = atom();
    let mixop = Mixfix::Seq(Vec::new());
    let var_il = (id("v"), typ(), vec![il::ast::Iter::List]);
    let value = il::ast::Value::new(
        il::ast::ValueKind::BoolV(true),
        il::ast::TypKind::BoolT,
        span("value"),
    );
    let iterexp = (il::ast::Iter::List, vec![var_il.clone()]);
    let path = il::ast::Path::new(
        il::ast::PathKind::RootP,
        il::ast::TypKind::BoolT,
        span("path"),
    );
    let tparam = id("T");
    let targ = Spanned::new(il::ast::TypKind::BoolT, span("targ"));
    let arg = arg_exp("x");
    assert!(
        [
            al::eq::eq_atom(&atom, &atom),
            al::eq::eq_atoms(std::slice::from_ref(&atom), std::slice::from_ref(&atom)),
            al::eq::eq_mixop(&mixop, &mixop),
            al::eq::eq_iter(il::ast::Iter::List, il::ast::Iter::List),
            al::eq::eq_iters(&[il::ast::Iter::List], &[il::ast::Iter::List]),
            al::eq::eq_var(&var_il, &var_il),
            al::eq::eq_vars(std::slice::from_ref(&var_il), std::slice::from_ref(&var_il)),
            al::eq::eq_typ(&typ(), &typ()),
            al::eq::eq_typs(&[typ()], &[typ()]),
            al::eq::eq_nottyp(&not_typ(), &not_typ()),
            al::eq::eq_value(&value, &value),
            al::eq::eq_values(std::slice::from_ref(&value), std::slice::from_ref(&value)),
            al::eq::eq_exps(&[variable("x")], &[variable("x")]),
            al::eq::eq_iterexp(&iterexp, &iterexp),
            al::eq::eq_iterexps(
                std::slice::from_ref(&iterexp),
                std::slice::from_ref(&iterexp)
            ),
            al::eq::eq_pattern(
                &il::ast::Pattern::ListP(il::ast::ListPattern::Nil),
                &il::ast::Pattern::ListP(il::ast::ListPattern::Nil)
            ),
            al::eq::eq_path(&path, &path),
            al::eq::eq_tparam(&tparam, &tparam),
            al::eq::eq_tparams(std::slice::from_ref(&tparam), std::slice::from_ref(&tparam)),
            al::eq::eq_arg(&arg, &arg),
            al::eq::eq_args(std::slice::from_ref(&arg), std::slice::from_ref(&arg)),
            al::eq::eq_targ(&targ, &targ),
            al::eq::eq_targs(std::slice::from_ref(&targ), std::slice::from_ref(&targ)),
        ]
        .into_iter()
        .all(|equal| equal)
    );
}

#[test]
fn al_equality_distinguishes_recursive_operands_variants_and_collection_rules() {
    let value = |kind| il::ast::Value::new(kind, il::ast::TypKind::BoolT, span("value"));
    let recursive = value(il::ast::ValueKind::ListV(vec![value(
        il::ast::ValueKind::StructV(vec![(atom(), value(il::ast::ValueKind::BoolV(true)))]),
    )]));
    let recursive_changed = value(il::ast::ValueKind::ListV(vec![value(
        il::ast::ValueKind::StructV(vec![(atom(), value(il::ast::ValueKind::BoolV(false)))]),
    )]));
    let expressions = [
        (variable("x"), variable("x"), true),
        (variable("x"), variable("y"), false),
        (
            expr(il::ast::ExpKind::TupleE(vec![variable("x")])),
            expr(il::ast::ExpKind::TupleE(vec![variable("x"), variable("y")])),
            false,
        ),
    ];
    for (left, right, expected) in expressions {
        assert_eq!(al::eq::eq_exp(&left, &right), expected);
    }
    assert!(al::eq::eq_value(&recursive, &recursive));
    assert!(!al::eq::eq_value(&recursive, &recursive_changed));
    assert!(!al::eq::eq_value(
        &value(il::ast::ValueKind::BoolV(true)),
        &value(il::ast::ValueKind::TextV("true".to_owned())),
    ));

    let root = || {
        il::ast::Path::new(
            il::ast::PathKind::RootP,
            il::ast::TypKind::BoolT,
            span("root"),
        )
    };
    let path_x = il::ast::Path::new(
        il::ast::PathKind::IdxP(Box::new(root()), Box::new(variable("x"))),
        il::ast::TypKind::BoolT,
        span("path-x"),
    );
    let path_y = il::ast::Path::new(
        il::ast::PathKind::IdxP(Box::new(root()), Box::new(variable("y"))),
        il::ast::TypKind::BoolT,
        span("path-y"),
    );
    assert!(al::eq::eq_path(&path_x, &path_x));
    assert!(!al::eq::eq_path(&path_x, &path_y));
    assert!(!al::eq::eq_pattern(
        &il::ast::Pattern::ListP(il::ast::ListPattern::Nil),
        &il::ast::Pattern::ListP(il::ast::ListPattern::Cons),
    ));

    let rule = |input| {
        Spanned::new(
            il::ast::PremKind::RulePr(id("r"), not_exp("x"), input),
            span("rule"),
        )
    };
    assert!(al::eq::eq_prem(&rule(vec![0]), &rule(vec![0])));
    assert!(!al::eq::eq_prem(&rule(vec![0]), &rule(vec![1])));
    assert!(!al::eq::eq_prem(
        &Spanned::new(il::ast::PremKind::IfPr(variable("x")), span("if")),
        &Spanned::new(il::ast::PremKind::DebugPr(variable("x")), span("debug")),
    ));
    let iterprem = |bound, bind| (il::ast::Iter::List, bound, bind);
    let x_var = (id("x"), typ(), Vec::new());
    let y_var = (id("y"), typ(), Vec::new());
    assert!(al::eq::eq_iterprem(
        &iterprem(vec![x_var.clone(), y_var.clone()], vec![x_var.clone()]),
        &iterprem(vec![y_var.clone(), x_var.clone()], vec![x_var.clone()]),
    ));
    assert!(!al::eq::eq_iterprem(
        &iterprem(vec![x_var.clone()], vec![x_var.clone()]),
        &iterprem(vec![y_var.clone()], vec![x_var.clone()]),
    ));
    assert!(!al::eq::eq_iterprem(
        &iterprem(vec![x_var.clone()], vec![x_var.clone()]),
        &iterprem(vec![x_var.clone()], vec![y_var.clone()]),
    ));
    assert!(!al::eq::eq_exps(
        &[variable("x"), variable("y")],
        &[variable("y"), variable("x")]
    ));
    assert!(al::eq::eq_vars(
        &[x_var.clone(), y_var.clone()],
        &[y_var, x_var],
    ));
    assert!(!al::eq::eq_values(
        std::slice::from_ref(&recursive),
        &[recursive_changed],
    ));
    assert!(!al::eq::eq_args(&[arg_exp("x")], &[arg_exp("y")]));
}

#[test]
fn free_expression_path_argument_and_premise_variants_collect_identifier_text() {
    let x = || Box::new(variable("x"));
    let expressions = vec![
        (expr(il::ast::ExpKind::BoolE(true)), ids(&[])),
        (
            expr(il::ast::ExpKind::NumE(num::T::Nat(0.into()))),
            ids(&[]),
        ),
        (expr(il::ast::ExpKind::TextE("text".to_owned())), ids(&[])),
        (variable("x"), ids(&["x"])),
        (
            expr(il::ast::ExpKind::UnE(
                il::ast::UnOp::NotOp,
                il::ast::OpTyp::BoolT,
                x(),
            )),
            ids(&["x"]),
        ),
        (
            expr(il::ast::ExpKind::BinE(
                il::ast::BinOp::AndOp,
                il::ast::OpTyp::BoolT,
                x(),
                x(),
            )),
            ids(&["x"]),
        ),
        (
            expr(il::ast::ExpKind::CmpE(
                il::ast::CmpOp::EqOp,
                il::ast::OpTyp::BoolT,
                x(),
                x(),
            )),
            ids(&["x"]),
        ),
        (expr(il::ast::ExpKind::UpCastE(typ(), x())), ids(&["x"])),
        (expr(il::ast::ExpKind::DownCastE(typ(), x())), ids(&["x"])),
        (
            expr(il::ast::ExpKind::SubE(
                x(),
                typ(),
                Box::new(il::ast::Subcheck::SkipSC),
            )),
            ids(&["x"]),
        ),
        (
            expr(il::ast::ExpKind::MatchE(
                x(),
                il::ast::Pattern::ListP(il::ast::ListPattern::Nil),
            )),
            ids(&["x"]),
        ),
        (
            expr(il::ast::ExpKind::TupleE(vec![variable("x")])),
            ids(&["x"]),
        ),
        (
            expr(il::ast::ExpKind::CaseE(Box::new(not_exp("x")))),
            ids(&["x"]),
        ),
        (
            expr(il::ast::ExpKind::StrE(vec![(atom(), variable("x"))])),
            ids(&["x"]),
        ),
        (expr(il::ast::ExpKind::OptE(Some(x()))), ids(&["x"])),
        (expr(il::ast::ExpKind::OptE(None)), ids(&[])),
        (
            expr(il::ast::ExpKind::ListE(vec![variable("x")])),
            ids(&["x"]),
        ),
        (expr(il::ast::ExpKind::ConsE(x(), x())), ids(&["x"])),
        (expr(il::ast::ExpKind::CatE(x(), x())), ids(&["x"])),
        (expr(il::ast::ExpKind::MemE(x(), x())), ids(&["x"])),
        (expr(il::ast::ExpKind::LenE(x())), ids(&["x"])),
        (expr(il::ast::ExpKind::DotE(x(), atom())), ids(&["x"])),
        (expr(il::ast::ExpKind::IdxE(x(), x())), ids(&["x"])),
        (expr(il::ast::ExpKind::SliceE(x(), x(), x())), ids(&["x"])),
        (
            expr(il::ast::ExpKind::UpdE(x(), path_with("x"), x())),
            ids(&["x"]),
        ),
        (
            expr(il::ast::ExpKind::CallE(
                id("call"),
                Vec::new(),
                vec![arg_exp("x")],
            )),
            ids(&["x"]),
        ),
        (
            expr(il::ast::ExpKind::IterE(
                x(),
                (il::ast::Iter::List, Vec::new()),
            )),
            ids(&["x"]),
        ),
    ];
    for (expression, expected) in expressions {
        assert_eq!(al::free::free_exp(&expression), expected);
    }

    let paths = vec![
        (
            il::ast::Path::new(
                il::ast::PathKind::RootP,
                il::ast::TypKind::BoolT,
                span("root"),
            ),
            ids(&[]),
        ),
        (path_with("x"), ids(&["x"])),
        (
            il::ast::Path::new(
                il::ast::PathKind::SliceP(
                    Box::new(path_with("x")),
                    Box::new(variable("y")),
                    Box::new(variable("z")),
                ),
                il::ast::TypKind::BoolT,
                span("slice"),
            ),
            ids(&["x", "y", "z"]),
        ),
        (
            il::ast::Path::new(
                il::ast::PathKind::DotP(Box::new(path_with("x")), atom()),
                il::ast::TypKind::BoolT,
                span("dot"),
            ),
            ids(&["x"]),
        ),
    ];
    for (path, expected) in paths {
        assert_eq!(al::free::free_path(&path), expected);
    }

    assert_eq!(al::free::free_arg(&arg_exp("x")), ids(&["x"]));
    assert_eq!(
        al::free::free_arg(&Spanned::new(il::ast::ArgKind::DefA(id("x")), span("def"))),
        ids(&[])
    );
    let premises = vec![
        (
            il::ast::PremKind::RulePr(id("r"), not_exp("x"), Vec::new()),
            ids(&["x"]),
        ),
        (il::ast::PremKind::IfPr(variable("x")), ids(&["x"])),
        (
            il::ast::PremKind::IfHoldPr(id("r"), not_exp("x")),
            ids(&["x"]),
        ),
        (
            il::ast::PremKind::IfNotHoldPr(id("r"), not_exp("x")),
            ids(&["x"]),
        ),
        (
            il::ast::PremKind::LetPr(variable("x"), variable("y")),
            ids(&["x", "y"]),
        ),
        (
            il::ast::PremKind::IterPr(
                Box::new(Spanned::new(
                    il::ast::PremKind::IfPr(variable("x")),
                    span("nested"),
                )),
                (il::ast::Iter::List, Vec::new(), Vec::new()),
            ),
            ids(&["x"]),
        ),
        (il::ast::PremKind::DebugPr(variable("x")), ids(&["x"])),
    ];
    for (premise, expected) in premises {
        assert_eq!(
            al::free::free_prem(&Spanned::new(premise, span("premise"))),
            expected
        );
    }
}

#[test]
fn free_al_shapes_and_definition_arms_are_exhaustive() {
    let premise = || Spanned::new(il::ast::PremKind::IfPr(variable("p")), span("premise"));
    let rule_match: al::ast::RuleMatch =
        (vec![variable("s")], vec![variable("i")], vec![premise()]);
    let rule_path: al::ast::RulePath = (id("rule"), vec![premise()], vec![variable("o")]);
    let group: al::ast::RuleGroup = Spanned::new(
        (id("group"), rule_match.clone(), vec![rule_path.clone()]),
        span("group"),
    );
    let else_group: al::ast::ElseGroup = Spanned::new(
        (id("else"), rule_match.clone(), rule_path.clone()),
        span("else"),
    );
    let clause: al::ast::Clause = Spanned::new(
        (vec![arg_exp("a")], variable("c"), vec![premise()]),
        span("clause"),
    );
    let table: al::ast::TableRow = Spanned::new(
        (
            vec![variable("signature")],
            vec![arg_exp("a")],
            variable("t"),
            vec![premise()],
        ),
        span("table"),
    );

    assert_eq!(al::free::free_rulematch(&rule_match), ids(&["s", "i", "p"]));
    assert_eq!(al::free::free_rulepath(&rule_path), ids(&["p", "o"]));
    assert_eq!(al::free::free_rulegroup(&group), ids(&["s", "i", "p", "o"]));
    assert_eq!(
        al::free::free_elsegroup(&else_group),
        ids(&["s", "i", "p", "o"])
    );
    assert_eq!(al::free::free_clause(&clause), ids(&["a", "c", "p"]));
    assert_eq!(al::free::free_tablerow(&table), ids(&["a", "t", "p"]));

    let def_type = Spanned::new(il::ast::DefTypKind::PlainT(typ()), span("def-type"));
    let definitions: Vec<(al::ast::Def, BTreeSet<String>)> = vec![
        (
            Spanned::new(
                al::ast::DefKind::ExternTypD(id("e"), Vec::new()),
                span("def"),
            ),
            ids(&[]),
        ),
        (
            Spanned::new(
                al::ast::DefKind::TypD(id("t"), Vec::new(), def_type, Vec::new()),
                span("def"),
            ),
            ids(&[]),
        ),
        (
            Spanned::new(
                al::ast::DefKind::VarD(id("v"), typ(), Vec::new()),
                span("def"),
            ),
            ids(&[]),
        ),
        (
            Spanned::new(
                al::ast::DefKind::ExternRelD(id("er"), not_typ(), Vec::new(), Vec::new()),
                span("def"),
            ),
            ids(&[]),
        ),
        (
            Spanned::new(
                al::ast::DefKind::RelD(
                    id("r"),
                    not_typ(),
                    Vec::new(),
                    vec![group],
                    Some(else_group),
                    Vec::new(),
                ),
                span("def"),
            ),
            ids(&["s", "i", "p", "o"]),
        ),
        (
            Spanned::new(
                al::ast::DefKind::ExternDecD(id("ed"), Vec::new(), Vec::new(), typ(), Vec::new()),
                span("def"),
            ),
            ids(&[]),
        ),
        (
            Spanned::new(
                al::ast::DefKind::BuiltinDecD(id("bd"), Vec::new(), Vec::new(), typ(), Vec::new()),
                span("def"),
            ),
            ids(&[]),
        ),
        (
            Spanned::new(
                al::ast::DefKind::TableDecD(id("td"), Vec::new(), typ(), vec![table], Vec::new()),
                span("def"),
            ),
            ids(&["a", "t", "p"]),
        ),
        (
            Spanned::new(
                al::ast::DefKind::FuncDecD(
                    id("fd"),
                    Vec::new(),
                    Vec::new(),
                    typ(),
                    vec![clause.clone()],
                    Some(clause),
                    Vec::new(),
                ),
                span("def"),
            ),
            ids(&["a", "c", "p"]),
        ),
    ];
    for (definition, expected) in definitions {
        assert_eq!(al::free::free_def(&definition), expected);
    }
}
