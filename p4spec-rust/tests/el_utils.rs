use std::collections::BTreeSet;

use p4spec_rust::{
    domain::{
        atom::Atom as DomainAtom,
        source::{Region, Spanned},
    },
    lang::el::{
        ast::{self, BinOp, ExpKind},
        free, print,
    },
};

fn span(file: &str) -> Region {
    Region::for_file(file)
}

fn id(name: &str, file: &str) -> ast::Id {
    Spanned::new(name.to_owned(), span(file))
}

fn exp(kind: ExpKind, file: &str) -> ast::Exp {
    Spanned::new(kind, span(file))
}

fn atom(source: &str) -> ast::Atom {
    Spanned::new(DomainAtom::from_source(source), span("atom.watsup"))
}

fn plain(kind: ast::PlainTypKind) -> ast::PlainTyp {
    Spanned::new(kind, span("type.watsup"))
}

fn bool_typ() -> ast::PlainTyp {
    plain(ast::PlainTypKind::BoolT)
}

fn param(kind: ast::ParamKind) -> ast::Param {
    Spanned::new(kind, span("param.watsup"))
}

fn prem(kind: ast::PremKind) -> ast::Prem {
    Spanned::new(kind, span("prem.watsup"))
}

fn definition(kind: ast::DefKind) -> ast::Def {
    Spanned::new(kind, span("def.watsup"))
}

#[test]
fn free_expression_ids_ignore_source_spans_and_render_in_source_order() {
    let expression = exp(
        ExpKind::BinE(
            Box::new(exp(ExpKind::VarE(id("left", "left.watsup")), "left.watsup")),
            BinOp::AddOp,
            Box::new(exp(
                ExpKind::VarE(id("right", "right.watsup")),
                "right.watsup",
            )),
        ),
        "root.watsup",
    );

    assert_eq!(
        free::free_id_exp(&expression),
        BTreeSet::from(["left".to_owned(), "right".to_owned()])
    );
    assert_eq!(print::string_of_exp(&expression), "left + right");
}

#[test]
fn free_collection_covers_paths_calls_premises_and_definition_bodies() {
    let variable = |name| {
        exp(
            ExpKind::VarE(id(name, "different-source.watsup")),
            "expr.watsup",
        )
    };
    let path = Spanned::new(
        ast::PathKind::SliceP(
            Box::new(Spanned::new(
                ast::PathKind::IdxP(
                    Box::new(Spanned::new(ast::PathKind::RootP, span("path.watsup"))),
                    Box::new(variable("index")),
                ),
                span("path.watsup"),
            )),
            Box::new(variable("low")),
            Box::new(variable("high")),
        ),
        span("path.watsup"),
    );
    let call = exp(
        ExpKind::CallE(
            id("defined", "definition.watsup"),
            vec![bool_typ()],
            vec![
                Spanned::new(
                    ast::ArgKind::DefA(id("not_free", "arg.watsup")),
                    span("arg.watsup"),
                ),
                Spanned::new(ast::ArgKind::ExpA(variable("argument")), span("arg.watsup")),
            ],
        ),
        "call.watsup",
    );
    let expression = exp(
        ExpKind::UpdE(Box::new(call), path, Box::new(variable("field"))),
        "update.watsup",
    );
    let iteration = prem(ast::PremKind::IterPr(
        Box::new(prem(ast::PremKind::VarPr(
            id("bound", "prem.watsup"),
            bool_typ(),
        ))),
        ast::Iter::List,
    ));
    let rule = Spanned::new(
        (
            id("relation", "rule.watsup"),
            id("", "rule.watsup"),
            expression.clone(),
            vec![iteration, prem(ast::PremKind::IfPr(variable("guard")))],
        ),
        span("rule.watsup"),
    );
    let function = definition(ast::DefKind::FuncDefD(
        id("function", "def.watsup"),
        vec![Spanned::new("T".to_owned(), span("def.watsup"))],
        vec![Spanned::new(
            ast::ArgKind::ExpA(variable("argument")),
            span("def.watsup"),
        )],
        variable("body"),
        vec![prem(ast::PremKind::DebugPr(variable("debug")))],
    ));

    assert_eq!(
        free::free_id_def(&definition(ast::DefKind::RuleGroupD(
            id("relation", "def.watsup"),
            id("group", "def.watsup"),
            vec![rule],
        ))),
        BTreeSet::from([
            "argument".to_owned(),
            "bound".to_owned(),
            "field".to_owned(),
            "guard".to_owned(),
            "high".to_owned(),
            "index".to_owned(),
            "low".to_owned(),
        ])
    );
    assert_eq!(
        free::free_id_def(&function),
        BTreeSet::from(["argument".to_owned(), "body".to_owned(), "debug".to_owned()])
    );
    assert_eq!(
        free::free_tid_param(&param(ast::ParamKind::DefP(
            id("f", "param.watsup"),
            vec![Spanned::new("T".to_owned(), span("param.watsup"))],
            vec![param(ast::ParamKind::ExpP(plain(ast::PlainTypKind::VarT(
                id("Nested", "type.watsup"),
                vec![],
            ))))],
            plain(ast::PlainTypKind::VarT(id("Result", "type.watsup"), vec![])),
        ))),
        BTreeSet::from(["Nested".to_owned(), "Result".to_owned(), "T".to_owned()])
    );
}

#[test]
fn printer_preserves_el_delimiters_precedence_hints_and_definition_separators() {
    let hint = ast::Hint {
        hintid: id("ignored", "hint.watsup"),
        hintexp: exp(
            ExpKind::VarE(id("also_ignored", "hint.watsup")),
            "hint.watsup",
        ),
    };
    let nested = exp(
        ExpKind::BinE(
            Box::new(exp(
                ExpKind::ParenE(Box::new(exp(
                    ExpKind::BinE(
                        Box::new(exp(ExpKind::VarE(id("a", "a")), "a")),
                        ast::BinOp::AddOp,
                        Box::new(exp(ExpKind::VarE(id("b", "b")), "b")),
                    ),
                    "inner",
                ))),
                "outer",
            )),
            ast::BinOp::MulOp,
            Box::new(exp(
                ExpKind::IterE(
                    Box::new(exp(ExpKind::VarE(id("c", "c")), "c")),
                    ast::Iter::Opt,
                ),
                "outer",
            )),
        ),
        "outer",
    );
    assert_eq!(print::string_of_exp(&nested), "(a + b) * c?");
    assert_eq!(
        print::string_of_exp(&exp(
            ExpKind::CallE(
                id("f", "call"),
                vec![plain(ast::PlainTypKind::TextT)],
                vec![Spanned::new(
                    ast::ArgKind::DefA(id("g", "call")),
                    span("call")
                )]
            ),
            "call"
        )),
        "$f<text>($g)"
    );
    assert_eq!(
        print::string_of_prem(&prem(ast::PremKind::IterPr(
            Box::new(prem(ast::PremKind::IfPr(exp(
                ExpKind::VarE(id("ready", "prem")),
                "prem"
            )))),
            ast::Iter::List,
        ))),
        "(if ready)*"
    );

    let not_typ = Spanned::new(ast::NotTypKind::AtomT(atom("TERM")), span("type"));
    let definitions = vec![
        definition(ast::DefKind::ExternSynD(
            id("Syntax", "def"),
            vec![hint.clone()],
        )),
        definition(ast::DefKind::SynD(vec![(
            id("Pair", "def"),
            vec![Spanned::new("T".to_owned(), span("def"))],
        )])),
        definition(ast::DefKind::TypD(
            id("Record", "def"),
            vec![],
            Spanned::new(
                ast::DefTypKind::StructTD(vec![(atom("field"), bool_typ(), vec![hint.clone()])]),
                span("def"),
            ),
            vec![hint.clone()],
        )),
        definition(ast::DefKind::VarD(
            id("value", "def"),
            bool_typ(),
            vec![hint.clone()],
        )),
        definition(ast::DefKind::ExternRelD(
            id("external", "def"),
            not_typ.clone(),
            vec![hint.clone()],
        )),
        definition(ast::DefKind::RelD(
            id("internal", "def"),
            not_typ,
            vec![hint.clone()],
        )),
        definition(ast::DefKind::ExternDecD(
            id("extern", "def"),
            vec![],
            vec![param(ast::ParamKind::ExpP(bool_typ()))],
            bool_typ(),
            vec![hint.clone()],
        )),
        definition(ast::DefKind::BuiltinDecD(
            id("builtin", "def"),
            vec![],
            vec![],
            bool_typ(),
            vec![hint.clone()],
        )),
        definition(ast::DefKind::TableDecD(
            id("table", "def"),
            vec![],
            bool_typ(),
            vec![hint.clone()],
        )),
        definition(ast::DefKind::FuncDecD(
            id("declared", "def"),
            vec![],
            vec![],
            bool_typ(),
            vec![hint.clone()],
        )),
        definition(ast::DefKind::TableDefD(
            id("rows", "def"),
            vec![Spanned::new(
                (
                    exp(ExpKind::VarE(id("pattern", "row")), "row"),
                    exp(ExpKind::VarE(id("body", "row")), "row"),
                ),
                span("row"),
            )],
        )),
        definition(ast::DefKind::FuncDefD(
            id("defined", "def"),
            vec![],
            vec![],
            exp(ExpKind::VarE(id("body", "def")), "def"),
            vec![prem(ast::PremKind::ElsePr)],
        )),
        definition(ast::DefKind::SepD),
    ];
    assert_eq!(
        print::string_of_spec(&definitions),
        "extern syntax Syntax\nsyntax Pair<T>\nsyntax Record = {field bool}\nvar value : bool\nextern relation external: TERM\nrelation internal: TERM\nextern dec $extern(bool) : bool\nbuiltin dec $builtin : bool\ntbl dec $table : bool\ndec $declared : bool\ntbl def $rows =\n  pattern => body\ndef $defined = body\n -- otherwise\n\n\n\n"
    );
}

#[test]
fn printer_matches_ocaml_byte_escaping_and_public_collection_helpers() {
    let escaped = "\"\\'\n\r\t\x08\x0c\x01é";
    assert_eq!(
        print::string_of_exp(&exp(ExpKind::TextE(escaped.into()), "text")),
        "\"\\\"\\\\'\\n\\r\\t\\b\\012\\001\\195\\169\""
    );
    assert_eq!(
        print::string_of_exp(&exp(ExpKind::LatexE(escaped.into()), "latex")),
        "latex(\\\"\\\\'\\n\\r\\t\\b\\012\\001\\195\\169)"
    );
    assert_eq!(print::string_of_unop(ast::UnOp::MinusOp), "-");
    assert_eq!(print::string_of_binop(ast::BinOp::EquivOp), "<=>");
    assert_eq!(print::string_of_cmpop(ast::CmpOp::NeOp), "=/=");
    let atom_type = Spanned::new(ast::NotTypKind::AtomT(atom("A")), span("type"));
    assert_eq!(
        print::string_of_nottyps(", ", &[atom_type.clone(), atom_type]),
        "A, A"
    );
    let row = Spanned::new(
        (exp(ExpKind::EpsE, "row"), exp(ExpKind::EpsE, "row")),
        span("row"),
    );
    assert_eq!(
        print::string_of_tablerows(&[row.clone(), row]),
        "eps => eps\n  | eps => eps"
    );
    let rule = Spanned::new(
        (
            id("r", "rule"),
            id("", "rule"),
            exp(ExpKind::EpsE, "rule"),
            vec![],
        ),
        span("rule"),
    );
    assert_eq!(
        print::string_of_rules(&[rule.clone(), rule]),
        "rule r:\n  eps\nrule r:\n  eps"
    );
}

#[test]
fn printer_tables_cover_remaining_el_constructor_families() {
    let var = |name| exp(ExpKind::VarE(id(name, "expr")), "expr");
    let expressions = [
        (
            exp(
                ExpKind::NumE(ast::NumOp::HexOp, ast::Num::Nat(15.into())),
                "expr",
            ),
            "0xF",
        ),
        (exp(ExpKind::ArithE(Box::new(var("x"))), "expr"), "$(x)"),
        (
            exp(ExpKind::ListE(vec![var("x"), var("y")]), "expr"),
            "[x, y]",
        ),
        (
            exp(
                ExpKind::ConsE(Box::new(var("x")), Box::new(var("xs"))),
                "expr",
            ),
            "x :: xs",
        ),
        (
            exp(
                ExpKind::CatE(Box::new(var("x")), Box::new(var("y"))),
                "expr",
            ),
            "x ++ y",
        ),
        (
            exp(
                ExpKind::SliceE(Box::new(var("x")), Box::new(var("i")), Box::new(var("j"))),
                "expr",
            ),
            "x[i : j]",
        ),
        (
            exp(ExpKind::StrE(vec![(atom("field"), var("x"))]), "expr"),
            "{field x}",
        ),
        (
            exp(
                ExpKind::InfixE(Box::new(var("x")), atom("'++'"), Box::new(var("y"))),
                "expr",
            ),
            "x '++' y",
        ),
        (
            exp(
                ExpKind::BrackE(atom("`("), Box::new(var("x")), atom("`)")),
                "expr",
            ),
            "``(x`)",
        ),
        (
            exp(
                ExpKind::FuseE(Box::new(var("x")), Box::new(var("y"))),
                "expr",
            ),
            "x#y",
        ),
    ];
    for (expression, expected) in expressions {
        assert_eq!(print::string_of_exp(&expression), expected);
    }

    let root = Spanned::new(ast::PathKind::RootP, span("path"));
    let paths = [
        (root.clone(), ""),
        (
            Spanned::new(
                ast::PathKind::IdxP(Box::new(root.clone()), Box::new(var("i"))),
                span("path"),
            ),
            "[i]",
        ),
        (
            Spanned::new(
                ast::PathKind::SliceP(
                    Box::new(root.clone()),
                    Box::new(var("i")),
                    Box::new(var("j")),
                ),
                span("path"),
            ),
            "[i : j]",
        ),
        (
            Spanned::new(
                ast::PathKind::DotP(Box::new(root), atom("field")),
                span("path"),
            ),
            "field",
        ),
    ];
    for (path, expected) in paths {
        assert_eq!(print::string_of_path(&path), expected);
    }

    let types = [
        (
            plain(ast::PlainTypKind::NumT(
                p4spec_rust::lang::xl::num::Typ::IntT,
            )),
            "int",
        ),
        (
            plain(ast::PlainTypKind::ParenT(Box::new(bool_typ()))),
            "(bool)",
        ),
        (
            plain(ast::PlainTypKind::TupleT(vec![bool_typ(), bool_typ()])),
            "(bool, bool)",
        ),
        (
            plain(ast::PlainTypKind::IterT(
                Box::new(bool_typ()),
                ast::Iter::List,
            )),
            "bool*",
        ),
    ];
    for (typ, expected) in types {
        assert_eq!(print::string_of_plaintyp(&typ), expected);
    }
    let notation = [
        (
            Spanned::new(
                ast::NotTypKind::SeqT(vec![
                    ast::Typ::PlainT(bool_typ()),
                    ast::Typ::PlainT(bool_typ()),
                ]),
                span("type"),
            ),
            "bool bool",
        ),
        (
            Spanned::new(
                ast::NotTypKind::InfixT(
                    Box::new(ast::Typ::PlainT(bool_typ())),
                    atom("'~'"),
                    Box::new(ast::Typ::PlainT(bool_typ())),
                ),
                span("type"),
            ),
            "bool '~' bool",
        ),
        (
            Spanned::new(
                ast::NotTypKind::BrackT(
                    atom("`["),
                    Box::new(ast::Typ::PlainT(bool_typ())),
                    atom("`]"),
                ),
                span("type"),
            ),
            "``[bool`]",
        ),
    ];
    for (typ, expected) in notation {
        assert_eq!(print::string_of_nottyp(&typ), expected);
    }
    let def_types = [
        (
            Spanned::new(ast::DefTypKind::PlainTD(bool_typ()), span("type")),
            "bool",
        ),
        (
            Spanned::new(
                ast::DefTypKind::VariantTD(vec![(ast::Typ::PlainT(bool_typ()), vec![])]),
                span("type"),
            ),
            "\n   | bool",
        ),
    ];
    for (typ, expected) in def_types {
        assert_eq!(print::string_of_deftyp(&typ), expected);
    }

    let premises = [
        (
            prem(ast::PremKind::VarPr(id("x", "prem"), bool_typ())),
            "x : bool",
        ),
        (
            prem(ast::PremKind::RulePr(id("r", "prem"), var("x"))),
            "r: x",
        ),
        (
            prem(ast::PremKind::RuleNotPr(id("r", "prem"), var("x"))),
            "r:/ x",
        ),
        (prem(ast::PremKind::DebugPr(var("x"))), "debug x"),
    ];
    for (premise, expected) in premises {
        assert_eq!(print::string_of_prem(&premise), expected);
    }
    let rule = Spanned::new(
        (id("r", "rule"), id("g", "rule"), var("x"), vec![]),
        span("rule"),
    );
    assert_eq!(
        print::string_of_def(&definition(ast::DefKind::RuleGroupD(
            id("r", "def"),
            id("g", "def"),
            vec![rule]
        ))),
        "rulegroup r/g:\n  rule r/g:\n  x"
    );
}
