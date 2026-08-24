use std::collections::{BTreeMap, BTreeSet};

use p4spec_rust::{
    domain::{
        atom::Atom,
        external_data::ExternalData,
        mixfix::Mixfix,
        source::{Region, Spanned},
    },
    lang::il::{ast, fresh, print},
};

fn typ() -> ast::Typ {
    Spanned::new(ast::TypKind::BoolT, Region::none())
}
fn id(name: &str) -> ast::Id {
    Spanned::new(name.into(), Region::none())
}
fn atom(name: &str) -> ast::Atom {
    Spanned::new(Atom::Keyword(name.into()), Region::none())
}
fn exp(kind: ast::ExpKind) -> ast::Exp {
    ast::Exp::new(kind, ast::TypKind::BoolT, Region::none())
}
fn var(name: &str) -> ast::Exp {
    exp(ast::ExpKind::VarE(id(name)))
}
fn arg(kind: ast::ArgKind) -> ast::Arg {
    Spanned::new(kind, Region::none())
}
fn prem(kind: ast::PremKind) -> ast::Prem {
    Spanned::new(kind, Region::none())
}
fn notexp(name: &str) -> ast::NotExp {
    Mixfix::Seq(vec![Mixfix::Arg(var(name))])
}
fn nottyp() -> ast::NotTyp {
    Spanned::new(Mixfix::Arg(typ()), Region::none())
}
fn names(names: &[&str]) -> BTreeSet<ast::IdKind> {
    names.iter().map(|name| (*name).into()).collect()
}
fn hint() -> ast::Hint {
    p4spec_rust::lang::el::ast::Hint {
        hintid: Spanned::new("meta".into(), Region::none()),
        hintexp: Spanned::new(
            p4spec_rust::lang::el::ast::ExpKind::VarE(Spanned::new(
                "payload".into(),
                Region::none(),
            )),
            Region::none(),
        ),
    }
}

#[test]
fn printer_tables_cover_il_constructor_families_and_escapes() {
    let nested_path = ast::Path::new(
        ast::PathKind::DotP(
            Box::new(ast::Path::new(
                ast::PathKind::IdxP(
                    Box::new(ast::Path::new(
                        ast::PathKind::RootP,
                        ast::TypKind::BoolT,
                        Region::none(),
                    )),
                    Box::new(var("index")),
                ),
                ast::TypKind::BoolT,
                Region::none(),
            )),
            atom("field"),
        ),
        ast::TypKind::BoolT,
        Region::none(),
    );
    let expressions = vec![
        ("bool", exp(ast::ExpKind::BoolE(true)), "true"),
        (
            "number",
            exp(ast::ExpKind::NumE(ast::Num::Int((-2).into()))),
            "-2",
        ),
        (
            "text",
            exp(ast::ExpKind::TextE("\"\\'\x08\t\n\r\u{00e9}".into())),
            "\"\\\"\\\\'\\b\\t\\n\\r\\195\\169\"",
        ),
        (
            "unary",
            exp(ast::ExpKind::UnE(
                ast::UnOp::NotOp,
                ast::OpTyp::BoolT,
                Box::new(var("x")),
            )),
            "~x",
        ),
        (
            "binary",
            exp(ast::ExpKind::BinE(
                ast::BinOp::AddOp,
                ast::OpTyp::NatT,
                Box::new(var("x")),
                Box::new(var("y")),
            )),
            "(x + y)",
        ),
        (
            "comparison",
            exp(ast::ExpKind::CmpE(
                ast::CmpOp::LeOp,
                ast::OpTyp::NatT,
                Box::new(var("x")),
                Box::new(var("y")),
            )),
            "(x <= y)",
        ),
        (
            "cast",
            exp(ast::ExpKind::UpCastE(typ(), Box::new(var("x")))),
            "x as bool",
        ),
        (
            "subtype",
            exp(ast::ExpKind::SubE(
                Box::new(var("x")),
                typ(),
                Box::new(ast::Subcheck::SkipSC),
            )),
            "x <: bool",
        ),
        (
            "match",
            exp(ast::ExpKind::MatchE(
                Box::new(var("x")),
                ast::Pattern::ListP(ast::ListPattern::Fixed(2)),
            )),
            "x matches [ _/2 ]",
        ),
        (
            "tuple",
            exp(ast::ExpKind::TupleE(vec![var("x"), var("y")])),
            "(x, y)",
        ),
        ("case", exp(ast::ExpKind::CaseE(Box::new(notexp("x")))), "x"),
        (
            "struct",
            exp(ast::ExpKind::StrE(vec![(atom("field"), var("x"))])),
            "{field x}",
        ),
        (
            "option",
            exp(ast::ExpKind::OptE(Some(Box::new(var("x"))))),
            "?(x)",
        ),
        ("empty_option", exp(ast::ExpKind::OptE(None)), "?()"),
        (
            "list",
            exp(ast::ExpKind::ListE(vec![var("x"), var("y")])),
            "[x, y]",
        ),
        (
            "cons",
            exp(ast::ExpKind::ConsE(Box::new(var("x")), Box::new(var("y")))),
            "x :: y",
        ),
        (
            "cat",
            exp(ast::ExpKind::CatE(Box::new(var("x")), Box::new(var("y")))),
            "x ++ y",
        ),
        (
            "mem",
            exp(ast::ExpKind::MemE(Box::new(var("x")), Box::new(var("y")))),
            "x <- y",
        ),
        ("len", exp(ast::ExpKind::LenE(Box::new(var("x")))), "|x|"),
        (
            "dot",
            exp(ast::ExpKind::DotE(Box::new(var("x")), atom("field"))),
            "x.field",
        ),
        (
            "idx",
            exp(ast::ExpKind::IdxE(Box::new(var("x")), Box::new(var("i")))),
            "x[i]",
        ),
        (
            "slice",
            exp(ast::ExpKind::SliceE(
                Box::new(var("x")),
                Box::new(var("l")),
                Box::new(var("h")),
            )),
            "x[l : h]",
        ),
        (
            "update",
            exp(ast::ExpKind::UpdE(
                Box::new(var("x")),
                nested_path,
                Box::new(var("value")),
            )),
            "x[[index].field = value]",
        ),
        (
            "call",
            exp(ast::ExpKind::CallE(
                id("f"),
                vec![typ()],
                vec![
                    arg(ast::ArgKind::DefA(id("g"))),
                    arg(ast::ArgKind::ExpA(var("x"))),
                ],
            )),
            "$f<bool>($g, x)",
        ),
        (
            "iter",
            exp(ast::ExpKind::IterE(
                Box::new(var("x")),
                (
                    ast::Iter::Opt,
                    vec![(id("z"), typ(), vec![ast::Iter::List])],
                ),
            )),
            "x?{z* <- z*?}",
        ),
    ];
    for (name, expression, expected) in expressions {
        assert_eq!(print::string_of_exp(&expression), expected, "{name}");
    }
    assert_eq!(
        print::string_of_text("\"\\'\x08\u{00e9}"),
        "\"\\'\x08\u{00e9}"
    );
    assert_eq!(
        print::string_of_value(&ast::Value::new(
            ast::ValueKind::TextV("\"\\'\x08\u{00e9}".into()),
            ast::TypKind::TextT,
            Region::none()
        )),
        "\\\"\\\\'\\b\\195\\169"
    );
    assert_eq!(
        print::string_of_path(&ast::Path::new(
            ast::PathKind::RootP,
            ast::TypKind::BoolT,
            Region::none()
        )),
        ""
    );
    assert_eq!(
        print::string_of_path(&ast::Path::new(
            ast::PathKind::DotP(
                Box::new(ast::Path::new(
                    ast::PathKind::RootP,
                    ast::TypKind::BoolT,
                    Region::none()
                )),
                atom("root")
            ),
            ast::TypKind::BoolT,
            Region::none()
        )),
        "root"
    );
}

#[test]
fn printer_renders_nested_premises_and_definition_spec_goldens() {
    let iteration = (
        ast::Iter::List,
        vec![(id("bound"), typ(), vec![])],
        vec![(id("output"), typ(), vec![ast::Iter::Opt])],
    );
    let nested = prem(ast::PremKind::IterPr(
        Box::new(prem(ast::PremKind::IterPr(
            Box::new(prem(ast::PremKind::IfPr(var("ready")))),
            iteration.clone(),
        ))),
        iteration,
    ));
    assert_eq!(
        print::string_of_prem(&nested),
        "(if ready)*{bound <- bound*, output? -> output?*}*{bound <- bound*, output? -> output?*}"
    );
    let rule = Spanned::new(
        (
            id("r"),
            notexp("head"),
            vec![
                prem(ast::PremKind::RulePr(
                    id("relation"),
                    notexp("input"),
                    vec![0],
                )),
                prem(ast::PremKind::LetPr(var("left"), var("right"))),
                nested,
            ],
        ),
        Region::none(),
    );
    let group = Spanned::new((id("main"), vec![rule.clone()]), Region::none());
    let else_group = Spanned::new((id("fallback"), rule.clone()), Region::none());
    let clause = Spanned::new(
        (
            vec![arg(ast::ArgKind::ExpA(var("argument")))],
            var("result"),
            vec![prem(ast::PremKind::DebugPr(var("debug")))],
        ),
        Region::none(),
    );
    let row = Spanned::new(
        (vec![arg(ast::ArgKind::ExpA(var("key")))], var("value")),
        Region::none(),
    );
    let definitions = vec![
        Spanned::new(
            ast::DefKind::ExternTypD(id("Syntax"), vec![]),
            Region::none(),
        ),
        Spanned::new(
            ast::DefKind::TypD(
                id("Alias"),
                vec![Spanned::new("T".into(), Region::none())],
                Spanned::new(
                    ast::DefTypKind::VariantT(vec![(
                        nottyp(),
                        Spanned::new((id("Origin"), vec![]), Region::none()),
                        vec![],
                    )]),
                    Region::none(),
                ),
                vec![],
            ),
            Region::none(),
        ),
        Spanned::new(
            ast::DefKind::VarD(id("value"), typ(), vec![]),
            Region::none(),
        ),
        Spanned::new(
            ast::DefKind::ExternRelD(id("external"), nottyp(), vec![], vec![]),
            Region::none(),
        ),
        Spanned::new(
            ast::DefKind::RelD(
                id("relation"),
                nottyp(),
                vec![],
                vec![group],
                Some(else_group),
                vec![],
            ),
            Region::none(),
        ),
        Spanned::new(
            ast::DefKind::ExternDecD(id("extern"), vec![], vec![], typ(), vec![]),
            Region::none(),
        ),
        Spanned::new(
            ast::DefKind::BuiltinDecD(id("builtin"), vec![], vec![], typ(), vec![]),
            Region::none(),
        ),
        Spanned::new(
            ast::DefKind::TableDecD(id("table"), vec![], typ(), vec![row], vec![]),
            Region::none(),
        ),
        Spanned::new(
            ast::DefKind::FuncDecD(
                id("function"),
                vec![],
                vec![],
                typ(),
                vec![clause.clone()],
                Some(clause),
                vec![],
            ),
            Region::none(),
        ),
    ];
    let rendered = print::string_of_spec(&definitions);
    assert_eq!(
        rendered,
        concat!(
            "extern syntax Syntax\n\n",
            "syntax Alias<T> = \n   | bool (from Origin) \n\n",
            "var value : bool\n\n",
            "extern relation external: bool\n\n",
            "relation relation: bool\n\n",
            "  rulegroup main\n\n",
            "    rule r: head\n",
            "    -- relation: input\n",
            "    -- let left = right\n",
            "    -- (if ready)*{bound <- bound*, output? -> output?*}*{bound <- bound*, output? -> output?*}\n\n",
            "  elsegroup\n\n",
            "  rulegroup fallback\n\n",
            "    rule r: head\n",
            "    -- relation: input\n",
            "    -- let left = right\n",
            "    -- (if ready)*{bound <- bound*, output? -> output?*}*{bound <- bound*, output? -> output?*}\n\n",
            "extern def $extern : bool\n\n",
            "builtin def $builtin : bool\n\n",
            "tbl def $table : bool =\n  row 0 :\n    (key) -> value\n\n",
            "def $function : bool =\n\n",
            "  clause 0 : (argument) = result\n",
            "  -- debug debug\n\n",
            "  clause -1 : (argument) = result\n",
            "  -- debug debug"
        )
    );
    assert_eq!(print::string_of_defs(&definitions), rendered);
    assert_eq!(print::string_of_hints(&[hint()]), " hint(meta payload)");
    assert_eq!(
        print::string_of_def(&Spanned::new(
            ast::DefKind::VarD(id("hidden_metadata"), typ(), vec![hint()]),
            Region::none(),
        )),
        "var hidden_metadata : bool"
    );
}

#[test]
fn fresh_uses_aliases_collisions_wildcards_and_dimensions_deterministically() {
    let at = Region::for_file("fresh");
    let ids = names(&["bool", "bool'", "bool_1"]);
    assert_eq!(
        fresh::id(&ids, &Spanned::new("bool".into(), at.clone())).node,
        "bool''"
    );
    let mut aliases = BTreeMap::new();
    aliases.insert("B".into(), typ());
    let alias = fresh::var_from_typ(&aliases, &BTreeSet::new(), at.clone(), &typ());
    assert_eq!(
        (
            alias.0.node.as_str(),
            alias.1.node.clone(),
            alias.2.as_slice()
        ),
        ("B", ast::TypKind::BoolT, &[] as &[ast::Iter])
    );
    aliases.insert("C".into(), typ());
    assert_eq!(
        fresh::var_from_typ(&aliases, &BTreeSet::new(), at.clone(), &typ())
            .0
            .node,
        "bool"
    );
    assert_eq!(
        fresh::var_from_typ_wildcard(&BTreeMap::new(), &BTreeSet::new(), at.clone(), &typ())
            .0
            .node,
        "_bool"
    );
    let nested = Spanned::new(
        ast::TypKind::IterT(
            Box::new(Spanned::new(
                ast::TypKind::IterT(Box::new(typ()), ast::Iter::Opt),
                at.clone(),
            )),
            ast::Iter::List,
        ),
        at.clone(),
    );
    let nested_var = fresh::var_from_typ(&BTreeMap::new(), &BTreeSet::new(), at.clone(), &nested);
    assert_eq!(nested_var.0.node, "bool");
    assert_eq!(nested_var.1.node, ast::TypKind::BoolT);
    assert_eq!(nested_var.2, vec![ast::Iter::Opt, ast::Iter::List]);
    let from_exp = fresh::var_from_exp(
        &BTreeMap::new(),
        &BTreeSet::new(),
        &ast::Exp::new(ast::ExpKind::BoolE(true), ast::TypKind::TextT, at.clone()),
    );
    assert_eq!(from_exp.0.span, at);
    assert_eq!(from_exp.0.node, "text");
}

#[test]
fn printer_tables_cover_remaining_public_arms() {
    let type_cases = vec![
        (Spanned::new(ast::TypKind::BoolT, Region::none()), "bool"),
        (
            Spanned::new(
                ast::TypKind::NumT(p4spec_rust::lang::xl::num::Typ::NatT),
                Region::none(),
            ),
            "nat",
        ),
        (
            Spanned::new(
                ast::TypKind::NumT(p4spec_rust::lang::xl::num::Typ::IntT),
                Region::none(),
            ),
            "int",
        ),
        (Spanned::new(ast::TypKind::TextT, Region::none()), "text"),
        (
            Spanned::new(ast::TypKind::VarT(id("T"), vec![typ()]), Region::none()),
            "T<bool>",
        ),
        (
            Spanned::new(ast::TypKind::TupleT(vec![typ(), typ()]), Region::none()),
            "(bool, bool)",
        ),
        (
            Spanned::new(
                ast::TypKind::IterT(Box::new(typ()), ast::Iter::List),
                Region::none(),
            ),
            "bool*",
        ),
        (
            Spanned::new(
                ast::TypKind::FuncT(
                    vec![Spanned::new("T".into(), Region::none())],
                    vec![typ()],
                    Box::new(typ()),
                ),
                Region::none(),
            ),
            "<T>(bool) : bool",
        ),
    ];
    for (typ, expected) in type_cases {
        assert_eq!(print::string_of_typ(&typ), expected);
    }
    let nottyp = Spanned::new(
        Mixfix::Infix(
            Box::new(Mixfix::Arg(typ())),
            atom("or"),
            Box::new(Mixfix::Arg(typ())),
        ),
        Region::none(),
    );
    assert_eq!(print::string_of_nottyp(&nottyp), "bool or bool");
    let origin = Spanned::new((id("Origin"), vec![typ()]), Region::none());
    let def_types = vec![
        (
            Spanned::new(ast::DefTypKind::PlainT(typ()), Region::none()),
            "bool",
        ),
        (
            Spanned::new(
                ast::DefTypKind::StructT(vec![(atom("field"), typ())]),
                Region::none(),
            ),
            "{field bool}",
        ),
        (
            Spanned::new(
                ast::DefTypKind::VariantT(vec![(nottyp.clone(), origin, vec![hint()])]),
                Region::none(),
            ),
            "\n   | bool or bool (from Origin<bool>)  hint(meta payload)",
        ),
    ];
    for (deftyp, expected) in def_types {
        assert_eq!(print::string_of_deftyp(&deftyp), expected);
    }
    assert_eq!(print::string_of_typs(",", &[]), "");
    assert_eq!(print::string_of_typfields(",", &[]), "");
    assert_eq!(print::string_of_typcases(",", &[]), "");
    let bool_value = |value| {
        ast::Value::new(
            ast::ValueKind::BoolV(value),
            ast::TypKind::BoolT,
            Region::none(),
        )
    };
    let values = vec![
        (
            ast::Value::new(
                ast::ValueKind::BoolV(true),
                ast::TypKind::BoolT,
                Region::none(),
            ),
            "true",
        ),
        (
            ast::Value::new(
                ast::ValueKind::NumV(ast::Num::Nat(2.into())),
                ast::TypKind::NumT(p4spec_rust::lang::xl::num::Typ::NatT),
                Region::none(),
            ),
            "2",
        ),
        (
            ast::Value::new(
                ast::ValueKind::StructV(vec![]),
                ast::TypKind::BoolT,
                Region::none(),
            ),
            "{}",
        ),
        (
            ast::Value::new(
                ast::ValueKind::TupleV(vec![bool_value(true)]),
                ast::TypKind::BoolT,
                Region::none(),
            ),
            "(true)",
        ),
        (
            ast::Value::new(
                ast::ValueKind::OptV(Some(Box::new(bool_value(true)))),
                ast::TypKind::BoolT,
                Region::none(),
            ),
            "Some(true)",
        ),
        (
            ast::Value::new(
                ast::ValueKind::OptV(None),
                ast::TypKind::BoolT,
                Region::none(),
            ),
            "None",
        ),
        (
            ast::Value::new(
                ast::ValueKind::ListV(vec![]),
                ast::TypKind::BoolT,
                Region::none(),
            ),
            "[]",
        ),
        (
            ast::Value::new(
                ast::ValueKind::FuncV(id("f")),
                ast::TypKind::BoolT,
                Region::none(),
            ),
            "$f",
        ),
        (
            ast::Value::new(
                ast::ValueKind::ExternV(ExternalData::Null),
                ast::TypKind::BoolT,
                Region::none(),
            ),
            "extern",
        ),
    ];
    for (value, expected) in values {
        assert_eq!(print::string_of_value(&value), expected);
    }
    let structured = ast::Value::new(
        ast::ValueKind::StructV(vec![(atom("field"), bool_value(true))]),
        ast::TypKind::BoolT,
        Region::none(),
    );
    let listed = ast::Value::new(
        ast::ValueKind::ListV(vec![bool_value(true)]),
        ast::TypKind::BoolT,
        Region::none(),
    );
    let cased = ast::Value::new(
        ast::ValueKind::CaseV(Box::new(Mixfix::Seq(vec![
            Mixfix::Atom(atom("TAG")),
            Mixfix::Arg(bool_value(true)),
        ]))),
        ast::TypKind::BoolT,
        Region::none(),
    );
    assert_eq!(
        print::string_of_value_with(&structured, false, 1),
        "{\n    field true\n  }"
    );
    assert_eq!(print::string_of_short_value(&structured), "{ .../1 }");
    assert_eq!(
        print::string_of_value_with(&listed, false, 1),
        "[\n    true\n  ]"
    );
    assert_eq!(print::string_of_short_value(&listed), "[ .../1 ]");
    assert_eq!(print::string_of_value(&cased), "TAG true");
    assert_eq!(print::string_of_short_value(&cased), "`TAG %`");
    for (operator, expected) in [
        (ast::UnOp::NotOp, "~"),
        (ast::UnOp::PlusOp, "+"),
        (ast::UnOp::MinusOp, "-"),
    ] {
        assert_eq!(print::string_of_unop(operator), expected);
    }
    for (operator, expected) in [
        (ast::BinOp::AndOp, "/\\"),
        (ast::BinOp::OrOp, "\\/"),
        (ast::BinOp::ImplOp, "=>"),
        (ast::BinOp::EquivOp, "<=>"),
        (ast::BinOp::AddOp, "+"),
        (ast::BinOp::SubOp, "-"),
        (ast::BinOp::MulOp, "*"),
        (ast::BinOp::DivOp, "/"),
        (ast::BinOp::ModOp, "\\"),
        (ast::BinOp::PowOp, "^"),
    ] {
        assert_eq!(print::string_of_binop(operator), expected);
    }
    for (operator, expected) in [
        (ast::CmpOp::EqOp, "="),
        (ast::CmpOp::NeOp, "=/="),
        (ast::CmpOp::LtOp, "<"),
        (ast::CmpOp::GtOp, ">"),
        (ast::CmpOp::LeOp, "<="),
        (ast::CmpOp::GeOp, ">="),
    ] {
        assert_eq!(print::string_of_cmpop(operator), expected);
    }
    let patterns = vec![
        (ast::Pattern::CaseP(Mixfix::Atom(atom("TAG"))), "`TAG`"),
        (ast::Pattern::ListP(ast::ListPattern::Cons), "_ :: _"),
        (ast::Pattern::ListP(ast::ListPattern::Fixed(3)), "[ _/3 ]"),
        (ast::Pattern::ListP(ast::ListPattern::Nil), "[]"),
        (ast::Pattern::OptP(ast::OptPattern::Some), "(_)"),
        (ast::Pattern::OptP(ast::OptPattern::None), "()"),
    ];
    for (pattern, expected) in patterns {
        assert_eq!(print::string_of_pattern(&pattern), expected);
    }
    let slice_path = ast::Path::new(
        ast::PathKind::SliceP(
            Box::new(ast::Path::new(
                ast::PathKind::RootP,
                ast::TypKind::BoolT,
                Region::none(),
            )),
            Box::new(var("low")),
            Box::new(var("high")),
        ),
        ast::TypKind::BoolT,
        Region::none(),
    );
    assert_eq!(print::string_of_path(&slice_path), "[low : high]");
    assert_eq!(
        print::string_of_exp(&exp(ast::ExpKind::DownCastE(typ(), Box::new(var("x"))))),
        "x as bool"
    );
    let premise_cases = vec![
        (
            prem(ast::PremKind::IfHoldPr(id("r"), notexp("x"))),
            "if r: x holds",
        ),
        (
            prem(ast::PremKind::IfNotHoldPr(id("r"), notexp("x"))),
            "if r: x does not hold",
        ),
    ];
    for (premise, expected) in premise_cases {
        assert_eq!(print::string_of_prem(&premise), expected);
    }
    assert_eq!(
        print::string_of_prems(&[prem(ast::PremKind::IfPr(var("ready")))]),
        "\n-- if ready"
    );
    assert_eq!(
        print::string_of_prems_with(1, &[prem(ast::PremKind::IfPr(var("ready")))]),
        "\n  -- if ready"
    );
    assert_eq!(print::string_of_params(&[]), "");
    assert_eq!(print::string_of_args(&[]), "");
    assert_eq!(print::string_of_tparams(&[]), "");
    assert_eq!(print::string_of_targs(&[]), "");
    assert_eq!(print::string_of_iterexps(&[]), "");
    assert_eq!(print::string_of_iterprems(&[]), "");
}

fn assert_iterated_exp(exp: &ast::Exp, dim: bool, id_span: &Region, typ_span: &Region) {
    let ast::Exp {
        kind: ast::ExpKind::IterE(inner, (ast::Iter::List, outer_binders)),
        ty,
        span,
    } = exp
    else {
        panic!("outer iteration")
    };
    assert_eq!(span, id_span);
    let ast::TypKind::IterT(outer_typ, ast::Iter::List) = ty else {
        panic!("outer type")
    };
    assert_eq!(&outer_typ.span, id_span);
    let ast::TypKind::IterT(base_typ, ast::Iter::Opt) = &outer_typ.node else {
        panic!("inner type")
    };
    assert_eq!(base_typ.node, ast::TypKind::BoolT);
    assert_eq!(&base_typ.span, id_span);
    let ast::Exp {
        kind: ast::ExpKind::IterE(base, (ast::Iter::Opt, inner_binders)),
        ty: inner_ty,
        span: inner_span,
    } = inner.as_ref()
    else {
        panic!("inner iteration")
    };
    assert_eq!(inner_span, id_span);
    assert!(
        matches!(inner_ty, ast::TypKind::IterT(typ, ast::Iter::Opt) if typ.node == ast::TypKind::BoolT && typ.span == *id_span)
    );
    assert!(matches!(base.kind, ast::ExpKind::VarE(_)));
    assert_eq!(base.span, *id_span);
    assert_eq!(base.ty, ast::TypKind::BoolT);
    match (dim, inner_binders.as_slice(), outer_binders.as_slice()) {
        (false, [], []) => {}
        (true, [(inner_id, inner_typ, inner_prior)], [(outer_id, outer_typ, outer_prior)]) => {
            assert_eq!(&inner_id.span, id_span);
            assert_eq!(inner_id.node, "bool");
            assert_eq!(&inner_typ.span, typ_span);
            assert!(inner_prior.is_empty());
            assert!(
                matches!(inner_typ.node, ast::TypKind::IterT(ref typ, ast::Iter::Opt) if typ.node == ast::TypKind::BoolT && typ.span == *id_span)
            );
            assert_eq!(&outer_id.span, id_span);
            assert_eq!(outer_id.node, "bool");
            assert_eq!(&outer_typ.span, typ_span);
            assert_eq!(outer_prior, &vec![ast::Iter::Opt]);
            assert!(
                matches!(outer_typ.node, ast::TypKind::IterT(ref typ, ast::Iter::List) if matches!(typ.node, ast::TypKind::IterT(ref base, ast::Iter::Opt) if base.node == ast::TypKind::BoolT && base.span == *id_span) && typ.span == *id_span)
            );
        }
        _ => panic!("binder shape"),
    }
}

#[test]
fn fresh_exact_edges_preserve_aliases_regions_and_full_dimension_shapes() {
    let at = Region::for_file("requested");
    let alias_typ = Spanned::new(ast::TypKind::BoolT, Region::for_file("alias_type"));
    let mut aliases = BTreeMap::new();
    aliases.insert("bool".into(), alias_typ.clone());
    let rejected = fresh::var_from_typ(&aliases, &BTreeSet::new(), at.clone(), &typ());
    assert_eq!(rejected.0.node, "bool");
    assert_eq!(rejected.0.span, at);
    assert_eq!(rejected.1.span, Region::none());
    aliases.clear();
    aliases.insert("Alias".into(), alias_typ.clone());
    let selected = fresh::var_from_typ(
        &aliases,
        &BTreeSet::new(),
        Region::for_file("requested_alias"),
        &typ(),
    );
    assert_eq!(selected.0.node, "Alias");
    assert_eq!(selected.0.span, Region::for_file("requested_alias"));
    assert_eq!(selected.1, alias_typ);
    assert!(selected.2.is_empty());
    let collision_ids = names(&["_bool", "_bool'"]);
    let wildcard = fresh::var_from_typ_wildcard(
        &BTreeMap::new(),
        &collision_ids,
        Region::for_file("wildcard"),
        &typ(),
    );
    assert_eq!(wildcard.0.node, "_bool''");
    assert_eq!(wildcard.0.span, Region::for_file("wildcard"));
    let iter_bool = Spanned::new(
        ast::TypKind::IterT(Box::new(typ()), ast::Iter::List),
        Region::for_file("iter_type"),
    );
    let inside_iter = fresh::var_from_typ(
        &aliases,
        &BTreeSet::new(),
        Region::for_file("inside_iter"),
        &iter_bool,
    );
    assert_eq!(inside_iter.0.node, "Alias");
    assert_eq!(inside_iter.0.span, Region::for_file("inside_iter"));
    assert_eq!(
        inside_iter.1,
        Spanned::new(ast::TypKind::BoolT, Region::for_file("alias_type"))
    );
    assert_eq!(inside_iter.2, vec![ast::Iter::List]);
    let from_exp = fresh::var_from_exp(
        &BTreeMap::new(),
        &BTreeSet::new(),
        &ast::Exp::new(
            ast::ExpKind::BoolE(true),
            iter_bool.node.clone(),
            Region::for_file("expression"),
        ),
    );
    assert_eq!(from_exp.0.node, "bool");
    assert_eq!(from_exp.0.span, Region::for_file("expression"));
    assert_eq!(from_exp.1.node, ast::TypKind::BoolT);
    assert_eq!(from_exp.2, vec![ast::Iter::List]);
    let base_typ = Spanned::new(ast::TypKind::BoolT, Region::for_file("base_type"));
    let nested = Spanned::new(
        ast::TypKind::IterT(
            Box::new(Spanned::new(
                ast::TypKind::IterT(Box::new(base_typ.clone()), ast::Iter::Opt),
                Region::for_file("nested_inner"),
            )),
            ast::Iter::List,
        ),
        Region::for_file("nested_type"),
    );
    for dim in [false, true] {
        let (ids, expression) =
            fresh::exp_from_typ(dim, &BTreeMap::new(), &BTreeSet::new(), &nested);
        assert_eq!(ids, names(&["bool"]));
        assert_iterated_exp(&expression, dim, &nested.span, &base_typ.span);
    }
}
