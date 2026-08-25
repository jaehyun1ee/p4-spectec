use std::collections::{BTreeMap, BTreeSet};

use p4spec_rust::{
    domain::{
        atom::Atom,
        mixfix::Mixfix,
        source::{Position, Span, Spanned},
    },
    lang::{
        hints::input::InputHint,
        il::{ast, fresh, print},
    },
    yojson::ExternalData,
};

fn typ() -> ast::Typ {
    Spanned::new(ast::TypKind::BoolT, Span::default())
}
fn id(name: &str) -> ast::Id {
    Spanned::new(name.into(), Span::default())
}
fn atom(name: &str) -> ast::Atom {
    Spanned::new(Atom::Keyword(name.into()), Span::default())
}
fn exp(kind: ast::ExpKind) -> ast::Exp {
    ast::Exp::new(kind, ast::TypKind::BoolT, Span::default())
}
fn var(name: &str) -> ast::Exp {
    exp(ast::ExpKind::VarE(id(name)))
}
fn arg(kind: ast::ArgKind) -> ast::Arg {
    Spanned::new(kind, Span::default())
}
fn prem(kind: ast::PremKind) -> ast::Prem {
    Spanned::new(kind, Span::default())
}
fn notexp(name: &str) -> ast::NotExp {
    Mixfix::Seq(vec![Mixfix::Arg(var(name))])
}
fn nottyp() -> ast::NotTyp {
    Spanned::new(Mixfix::Arg(typ()), Span::default())
}
fn names(names: &[&str]) -> BTreeSet<ast::IdKind> {
    names.iter().map(|name| (*name).into()).collect()
}
fn hint() -> ast::Hint {
    p4spec_rust::lang::el::ast::Hint {
        hintid: Spanned::new("meta".into(), Span::default()),
        hintexp: Spanned::new(
            p4spec_rust::lang::el::ast::ExpKind::VarE(Spanned::new(
                "payload".into(),
                Span::default(),
            )),
            Span::default(),
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
                        Span::default(),
                    )),
                    Box::new(var("index")),
                ),
                ast::TypKind::BoolT,
                Span::default(),
            )),
            atom("field"),
        ),
        ast::TypKind::BoolT,
        Span::default(),
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
                    arg(ast::ArgKind::ExpA(Box::new(var("x")))),
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
                    vec![ast::Var {
                        id: id("z"),
                        typ: typ(),
                        iters: vec![ast::Iter::List],
                    }],
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
            Span::default()
        )),
        "\\\"\\\\'\\b\\195\\169"
    );
    assert_eq!(
        print::string_of_path(&ast::Path::new(
            ast::PathKind::RootP,
            ast::TypKind::BoolT,
            Span::default()
        )),
        ""
    );
    assert_eq!(
        print::string_of_path(&ast::Path::new(
            ast::PathKind::DotP(
                Box::new(ast::Path::new(
                    ast::PathKind::RootP,
                    ast::TypKind::BoolT,
                    Span::default()
                )),
                atom("root")
            ),
            ast::TypKind::BoolT,
            Span::default()
        )),
        "root"
    );
}

#[test]
fn printer_renders_nested_premises_and_definition_spec_goldens() {
    let iteration = ast::IterPrem {
        iter: ast::Iter::List,
        vars_bound: vec![ast::Var {
            id: id("bound"),
            typ: typ(),
            iters: vec![],
        }],
        vars_bind: vec![ast::Var {
            id: id("output"),
            typ: typ(),
            iters: vec![ast::Iter::Opt],
        }],
    };
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
        ast::RuleKind {
            id: id("r"),
            notation: notexp("head"),
            premises: vec![
                prem(ast::PremKind::RulePr(
                    id("relation"),
                    notexp("input"),
                    InputHint::new(vec![0]),
                )),
                prem(ast::PremKind::LetPr(var("left"), var("right"))),
                nested,
            ],
        },
        Span::default(),
    );
    let group = Spanned::new((id("main"), vec![rule.clone()]), Span::default());
    let else_group = Spanned::new((id("fallback"), rule.clone()), Span::default());
    let clause = Spanned::new(
        ast::ClauseKind {
            args: vec![arg(ast::ArgKind::ExpA(Box::new(var("argument"))))],
            expression: var("result"),
            premises: vec![prem(ast::PremKind::DebugPr(var("debug")))],
        },
        Span::default(),
    );
    let row = Spanned::new(
        (
            vec![arg(ast::ArgKind::ExpA(Box::new(var("key"))))],
            var("value"),
        ),
        Span::default(),
    );
    let definitions = vec![
        Spanned::new(
            ast::DefKind::ExternTypD(id("Syntax"), vec![]),
            Span::default(),
        ),
        Spanned::new(
            ast::DefKind::TypD(
                id("Alias"),
                vec![Spanned::new("T".into(), Span::default())],
                Spanned::new(
                    ast::DefTypKind::VariantT(vec![ast::TypCase {
                        notation: nottyp(),
                        origin: Spanned::new((id("Origin"), vec![]), Span::default()),
                        hints: vec![],
                    }]),
                    Span::default(),
                ),
                vec![],
            ),
            Span::default(),
        ),
        Spanned::new(
            ast::DefKind::VarD(id("value"), typ(), vec![]),
            Span::default(),
        ),
        Spanned::new(
            ast::DefKind::ExternRelD(id("external"), nottyp(), InputHint::new(vec![]), vec![]),
            Span::default(),
        ),
        Spanned::new(
            ast::DefKind::RelD(
                id("relation"),
                nottyp(),
                InputHint::new(vec![]),
                vec![group],
                Some(else_group),
                vec![],
            ),
            Span::default(),
        ),
        Spanned::new(
            ast::DefKind::ExternDecD(id("extern"), vec![], vec![], typ(), vec![]),
            Span::default(),
        ),
        Spanned::new(
            ast::DefKind::BuiltinDecD(id("builtin"), vec![], vec![], typ(), vec![]),
            Span::default(),
        ),
        Spanned::new(
            ast::DefKind::TableDecD(id("table"), vec![], typ(), vec![row], vec![]),
            Span::default(),
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
            Span::default(),
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
            Span::default(),
        )),
        "var hidden_metadata : bool"
    );
}

#[test]
fn fresh_uses_aliases_collisions_wildcards_and_dimensions_deterministically() {
    let at = Span::new(Position::new("fresh", 0, 0), Position::new("fresh", 0, 0));
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
            alias.id.node.as_str(),
            alias.typ.node.clone(),
            alias.iters.as_slice()
        ),
        ("B", ast::TypKind::BoolT, &[] as &[ast::Iter])
    );
    aliases.insert("C".into(), typ());
    assert_eq!(
        fresh::var_from_typ(&aliases, &BTreeSet::new(), at.clone(), &typ())
            .id
            .node,
        "bool"
    );
    assert_eq!(
        fresh::var_from_typ_wildcard(&BTreeMap::new(), &BTreeSet::new(), at.clone(), &typ())
            .id
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
    assert_eq!(nested_var.id.node, "bool");
    assert_eq!(nested_var.typ.node, ast::TypKind::BoolT);
    assert_eq!(nested_var.iters, vec![ast::Iter::Opt, ast::Iter::List]);
    let from_exp = fresh::var_from_exp(
        &BTreeMap::new(),
        &BTreeSet::new(),
        &ast::Exp::new(ast::ExpKind::BoolE(true), ast::TypKind::TextT, at.clone()),
    );
    assert_eq!(from_exp.id.span, at);
    assert_eq!(from_exp.id.node, "text");
}

#[test]
fn printer_tables_cover_remaining_public_arms() {
    let type_cases = vec![
        (Spanned::new(ast::TypKind::BoolT, Span::default()), "bool"),
        (
            Spanned::new(
                ast::TypKind::NumT(p4spec_rust::lang::xl::num::Typ::NatT),
                Span::default(),
            ),
            "nat",
        ),
        (
            Spanned::new(
                ast::TypKind::NumT(p4spec_rust::lang::xl::num::Typ::IntT),
                Span::default(),
            ),
            "int",
        ),
        (Spanned::new(ast::TypKind::TextT, Span::default()), "text"),
        (
            Spanned::new(ast::TypKind::VarT(id("T"), vec![typ()]), Span::default()),
            "T<bool>",
        ),
        (
            Spanned::new(ast::TypKind::TupleT(vec![typ(), typ()]), Span::default()),
            "(bool, bool)",
        ),
        (
            Spanned::new(
                ast::TypKind::IterT(Box::new(typ()), ast::Iter::List),
                Span::default(),
            ),
            "bool*",
        ),
        (
            Spanned::new(
                ast::TypKind::FuncT(
                    vec![Spanned::new("T".into(), Span::default())],
                    vec![typ()],
                    Box::new(typ()),
                ),
                Span::default(),
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
        Span::default(),
    );
    assert_eq!(print::string_of_nottyp(&nottyp), "bool or bool");
    let origin = Spanned::new((id("Origin"), vec![typ()]), Span::default());
    let def_types = vec![
        (
            Spanned::new(ast::DefTypKind::PlainT(typ()), Span::default()),
            "bool",
        ),
        (
            Spanned::new(
                ast::DefTypKind::StructT(vec![(atom("field"), typ())]),
                Span::default(),
            ),
            "{field bool}",
        ),
        (
            Spanned::new(
                ast::DefTypKind::VariantT(vec![ast::TypCase {
                    notation: nottyp.clone(),
                    origin,
                    hints: vec![hint()],
                }]),
                Span::default(),
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
            Span::default(),
        )
    };
    let values = vec![
        (
            ast::Value::new(
                ast::ValueKind::BoolV(true),
                ast::TypKind::BoolT,
                Span::default(),
            ),
            "true",
        ),
        (
            ast::Value::new(
                ast::ValueKind::NumV(ast::Num::Nat(2.into())),
                ast::TypKind::NumT(p4spec_rust::lang::xl::num::Typ::NatT),
                Span::default(),
            ),
            "2",
        ),
        (
            ast::Value::new(
                ast::ValueKind::StructV(vec![]),
                ast::TypKind::BoolT,
                Span::default(),
            ),
            "{}",
        ),
        (
            ast::Value::new(
                ast::ValueKind::TupleV(vec![bool_value(true)]),
                ast::TypKind::BoolT,
                Span::default(),
            ),
            "(true)",
        ),
        (
            ast::Value::new(
                ast::ValueKind::OptV(Some(Box::new(bool_value(true)))),
                ast::TypKind::BoolT,
                Span::default(),
            ),
            "Some(true)",
        ),
        (
            ast::Value::new(
                ast::ValueKind::OptV(None),
                ast::TypKind::BoolT,
                Span::default(),
            ),
            "None",
        ),
        (
            ast::Value::new(
                ast::ValueKind::ListV(vec![]),
                ast::TypKind::BoolT,
                Span::default(),
            ),
            "[]",
        ),
        (
            ast::Value::new(
                ast::ValueKind::FuncV(id("f")),
                ast::TypKind::BoolT,
                Span::default(),
            ),
            "$f",
        ),
        (
            ast::Value::new(
                ast::ValueKind::ExternV(ExternalData::Null),
                ast::TypKind::BoolT,
                Span::default(),
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
        Span::default(),
    );
    let listed = ast::Value::new(
        ast::ValueKind::ListV(vec![bool_value(true)]),
        ast::TypKind::BoolT,
        Span::default(),
    );
    let cased = ast::Value::new(
        ast::ValueKind::CaseV(Box::new(Mixfix::Seq(vec![
            Mixfix::Atom(atom("TAG")),
            Mixfix::Arg(bool_value(true)),
        ]))),
        ast::TypKind::BoolT,
        Span::default(),
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
    assert_eq!(print::string_of_short_value(&cased), "TAG %");
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
        (
            ast::Pattern::CaseP(Box::new(Mixfix::Atom(atom("TAG")))),
            "TAG",
        ),
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
                Span::default(),
            )),
            Box::new(var("low")),
            Box::new(var("high")),
        ),
        ast::TypKind::BoolT,
        Span::default(),
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

fn assert_iterated_exp(exp: &ast::Exp, dim: bool, id_span: &Span, typ_span: &Span) {
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
        (true, [inner], [outer]) => {
            assert_eq!(&inner.id.span, id_span);
            assert_eq!(inner.id.node, "bool");
            assert_eq!(&inner.typ.span, typ_span);
            assert!(inner.iters.is_empty());
            assert!(
                matches!(inner.typ.node, ast::TypKind::IterT(ref typ, ast::Iter::Opt) if typ.node == ast::TypKind::BoolT && typ.span == *id_span)
            );
            assert_eq!(&outer.id.span, id_span);
            assert_eq!(outer.id.node, "bool");
            assert_eq!(&outer.typ.span, typ_span);
            assert_eq!(outer.iters, vec![ast::Iter::Opt]);
            assert!(
                matches!(outer.typ.node, ast::TypKind::IterT(ref typ, ast::Iter::List) if matches!(typ.node, ast::TypKind::IterT(ref base, ast::Iter::Opt) if base.node == ast::TypKind::BoolT && base.span == *id_span) && typ.span == *id_span)
            );
        }
        _ => panic!("binder shape"),
    }
}

#[test]
fn fresh_exact_edges_preserve_aliases_regions_and_full_dimension_shapes() {
    let at = Span::new(
        Position::new("requested", 0, 0),
        Position::new("requested", 0, 0),
    );
    let alias_typ = Spanned::new(
        ast::TypKind::BoolT,
        Span::new(
            Position::new("alias_type", 0, 0),
            Position::new("alias_type", 0, 0),
        ),
    );
    let mut aliases = BTreeMap::new();
    aliases.insert("bool".into(), alias_typ.clone());
    let rejected = fresh::var_from_typ(&aliases, &BTreeSet::new(), at.clone(), &typ());
    assert_eq!(rejected.id.node, "bool");
    assert_eq!(rejected.id.span, at);
    assert_eq!(rejected.typ.span, Span::default());
    aliases.clear();
    aliases.insert("Alias".into(), alias_typ.clone());
    let selected = fresh::var_from_typ(
        &aliases,
        &BTreeSet::new(),
        Span::new(
            Position::new("requested_alias", 0, 0),
            Position::new("requested_alias", 0, 0),
        ),
        &typ(),
    );
    assert_eq!(selected.id.node, "Alias");
    assert_eq!(
        selected.id.span,
        Span::new(
            Position::new("requested_alias", 0, 0),
            Position::new("requested_alias", 0, 0)
        )
    );
    assert_eq!(selected.typ, alias_typ);
    assert!(selected.iters.is_empty());
    let collision_ids = names(&["_bool", "_bool'"]);
    let wildcard = fresh::var_from_typ_wildcard(
        &BTreeMap::new(),
        &collision_ids,
        Span::new(
            Position::new("wildcard", 0, 0),
            Position::new("wildcard", 0, 0),
        ),
        &typ(),
    );
    assert_eq!(wildcard.id.node, "_bool''");
    assert_eq!(
        wildcard.id.span,
        Span::new(
            Position::new("wildcard", 0, 0),
            Position::new("wildcard", 0, 0)
        )
    );
    let iter_bool = Spanned::new(
        ast::TypKind::IterT(Box::new(typ()), ast::Iter::List),
        Span::new(
            Position::new("iter_type", 0, 0),
            Position::new("iter_type", 0, 0),
        ),
    );
    let inside_iter = fresh::var_from_typ(
        &aliases,
        &BTreeSet::new(),
        Span::new(
            Position::new("inside_iter", 0, 0),
            Position::new("inside_iter", 0, 0),
        ),
        &iter_bool,
    );
    assert_eq!(inside_iter.id.node, "Alias");
    assert_eq!(
        inside_iter.id.span,
        Span::new(
            Position::new("inside_iter", 0, 0),
            Position::new("inside_iter", 0, 0)
        )
    );
    assert_eq!(
        inside_iter.typ,
        Spanned::new(
            ast::TypKind::BoolT,
            Span::new(
                Position::new("alias_type", 0, 0),
                Position::new("alias_type", 0, 0)
            )
        )
    );
    assert_eq!(inside_iter.iters, vec![ast::Iter::List]);
    let from_exp = fresh::var_from_exp(
        &BTreeMap::new(),
        &BTreeSet::new(),
        &ast::Exp::new(
            ast::ExpKind::BoolE(true),
            iter_bool.node.clone(),
            Span::new(
                Position::new("expression", 0, 0),
                Position::new("expression", 0, 0),
            ),
        ),
    );
    assert_eq!(from_exp.id.node, "bool");
    assert_eq!(
        from_exp.id.span,
        Span::new(
            Position::new("expression", 0, 0),
            Position::new("expression", 0, 0)
        )
    );
    assert_eq!(from_exp.typ.node, ast::TypKind::BoolT);
    assert_eq!(from_exp.iters, vec![ast::Iter::List]);
    let base_typ = Spanned::new(
        ast::TypKind::BoolT,
        Span::new(
            Position::new("base_type", 0, 0),
            Position::new("base_type", 0, 0),
        ),
    );
    let nested = Spanned::new(
        ast::TypKind::IterT(
            Box::new(Spanned::new(
                ast::TypKind::IterT(Box::new(base_typ.clone()), ast::Iter::Opt),
                Span::new(
                    Position::new("nested_inner", 0, 0),
                    Position::new("nested_inner", 0, 0),
                ),
            )),
            ast::Iter::List,
        ),
        Span::new(
            Position::new("nested_type", 0, 0),
            Position::new("nested_type", 0, 0),
        ),
    );
    for dim in [false, true] {
        let (ids, expression) =
            fresh::exp_from_typ(dim, &BTreeMap::new(), &BTreeSet::new(), &nested);
        assert_eq!(ids, names(&["bool"]));
        assert_iterated_exp(&expression, dim, &nested.span, &base_typ.span);
    }
}
