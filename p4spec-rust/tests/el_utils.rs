use p4spec_rust::{
    domain::{
        atom::Atom as DomainAtom,
        source::{Position, Span, Spanned},
    },
    lang::{
        common::ds::set::IdSet,
        el::{
            ast::{self, BinOp, ExpKind},
            free, print,
        },
    },
};

fn span(file: &str) -> Span {
    Span::new(Position::new(file, 0, 0), Position::new(file, 0, 0))
}

fn id(name: &str, file: &str) -> ast::Id {
    Spanned::new(name.to_owned(), span(file))
}

fn ids(names: &[&str]) -> IdSet {
    names
        .iter()
        .map(|name| id(name, "expected.watsup"))
        .collect()
}

fn exp(kind: ExpKind, file: &str) -> ast::Exp {
    Spanned::new(kind, span(file))
}

fn atom(source: &str) -> ast::Atom {
    Spanned::new(DomainAtom::of_string(source), span("atom.watsup"))
}

fn plain(kind: ast::PlainTypKind) -> ast::PlainTyp {
    Spanned::new(kind, span("type.watsup"))
}

fn bool_typ() -> ast::PlainTyp {
    plain(ast::PlainTypKind::Bool)
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
        ExpKind::Bin(
            Box::new(exp(ExpKind::Var(id("left", "left.watsup")), "left.watsup")),
            BinOp::Num(p4spec_rust::lang::xl::num::BinOp::Add),
            Box::new(exp(
                ExpKind::Var(id("right", "right.watsup")),
                "right.watsup",
            )),
        ),
        "root.watsup",
    );

    assert_eq!(free::free_id_exp(&expression), ids(&["left", "right"]));
    assert_eq!(print::string_of_exp(&expression), "left + right");
}

#[test]
fn free_collection_covers_paths_calls_premises_and_definition_bodies() {
    let variable = |name| {
        exp(
            ExpKind::Var(id(name, "different-source.watsup")),
            "expr.watsup",
        )
    };
    let path = Spanned::new(
        ast::PathKind::Slice(
            Box::new(Spanned::new(
                ast::PathKind::Idx(
                    Box::new(Spanned::new(ast::PathKind::Root, span("path.watsup"))),
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
        ExpKind::Call(
            id("defined", "definition.watsup"),
            vec![bool_typ()],
            vec![
                Spanned::new(
                    ast::ArgKind::Def(id("not_free", "arg.watsup")),
                    span("arg.watsup"),
                ),
                Spanned::new(
                    ast::ArgKind::Exp(Box::new(variable("argument"))),
                    span("arg.watsup"),
                ),
            ],
        ),
        "call.watsup",
    );
    let expression = exp(
        ExpKind::Upd(Box::new(call), path, Box::new(variable("field"))),
        "update.watsup",
    );
    let iteration = prem(ast::PremKind::Iter(ast::IterPrem {
        prem: Box::new(prem(ast::PremKind::Var(ast::VarPrem {
            id: id("bound", "prem.watsup"),
            plain_typ: bool_typ(),
        }))),
        iter: ast::Iter::List,
    }));
    let rule = Spanned::new(
        (
            id("relation", "rule.watsup"),
            id("", "rule.watsup"),
            expression.clone(),
            vec![
                iteration,
                prem(ast::PremKind::If(ast::IfPrem {
                    exp: variable("guard"),
                })),
            ],
        ),
        span("rule.watsup"),
    );
    let function = definition(ast::DefKind::FuncDef(ast::FuncDef {
        id: id("function", "def.watsup"),
        tparams: vec![Spanned::new("T".to_owned(), span("def.watsup"))],
        args: vec![Spanned::new(
            ast::ArgKind::Exp(Box::new(variable("argument"))),
            span("def.watsup"),
        )],
        exp: variable("body"),
        prems: vec![prem(ast::PremKind::Debug(ast::DebugPrem {
            exp: variable("debug"),
        }))],
    }));

    assert_eq!(
        free::free_id_def(&definition(ast::DefKind::RuleGroup(ast::RuleGroupDef {
            relid: id("relation", "def.watsup"),
            groupid: id("group", "def.watsup"),
            rules: vec![rule],
        }))),
        ids(&[
            "argument", "bound", "field", "guard", "high", "index", "low"
        ])
    );
    assert_eq!(
        free::free_id_def(&function),
        ids(&["argument", "body", "debug"])
    );
}

#[test]
fn printer_preserves_el_delimiters_precedence_hints_and_definition_separators() {
    let hint = (
        id("ignored", "hint.watsup"),
        exp(
            ExpKind::Var(id("also_ignored", "hint.watsup")),
            "hint.watsup",
        ),
    );
    let nested = exp(
        ExpKind::Bin(
            Box::new(exp(
                ExpKind::Paren(Box::new(exp(
                    ExpKind::Bin(
                        Box::new(exp(ExpKind::Var(id("a", "a")), "a")),
                        ast::BinOp::Num(p4spec_rust::lang::xl::num::BinOp::Add),
                        Box::new(exp(ExpKind::Var(id("b", "b")), "b")),
                    ),
                    "inner",
                ))),
                "outer",
            )),
            ast::BinOp::Num(p4spec_rust::lang::xl::num::BinOp::Mul),
            Box::new(exp(
                ExpKind::Iter(
                    Box::new(exp(ExpKind::Var(id("c", "c")), "c")),
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
            ExpKind::Call(
                id("f", "call"),
                vec![plain(ast::PlainTypKind::Text)],
                vec![Spanned::new(
                    ast::ArgKind::Def(id("g", "call")),
                    span("call")
                )]
            ),
            "call"
        )),
        "$f<text>($g)"
    );
    assert_eq!(
        print::string_of_prem(&prem(ast::PremKind::Iter(ast::IterPrem {
            prem: Box::new(prem(ast::PremKind::If(ast::IfPrem {
                exp: exp(ExpKind::Var(id("ready", "prem")), "prem"),
            }))),
            iter: ast::Iter::List,
        }))),
        "(if ready)*"
    );

    let not_typ = Spanned::new(ast::NotTypKind::Atom(atom("TERM")), span("type"));
    let definitions = vec![
        definition(ast::DefKind::ExternSyntax(ast::ExternSyntaxDef {
            id: id("Syntax", "def"),
            hints: vec![hint.clone()],
        })),
        definition(ast::DefKind::Syntax(ast::SyntaxDef {
            entries: vec![ast::SyntaxDefEntry {
                id: id("Pair", "def"),
                tparams: vec![Spanned::new("T".to_owned(), span("def"))],
            }],
        })),
        definition(ast::DefKind::Typ(ast::TypDef {
            id: id("Record", "def"),
            tparams: vec![],
            def_typ: Spanned::new(
                ast::DefTypKind::Struct(vec![(atom("field"), bool_typ(), vec![hint.clone()])]),
                span("def"),
            ),
            hints: vec![hint.clone()],
        })),
        definition(ast::DefKind::Var(ast::VarDef {
            id: id("value", "def"),
            plain_typ: bool_typ(),
            hints: vec![hint.clone()],
        })),
        definition(ast::DefKind::ExternRel(ast::ExternRelDef {
            id: id("external", "def"),
            not_typ: not_typ.clone(),
            hints: vec![hint.clone()],
        })),
        definition(ast::DefKind::Rel(ast::RelDef {
            id: id("internal", "def"),
            not_typ,
            hints: vec![hint.clone()],
        })),
        definition(ast::DefKind::ExternDec(ast::ExternDecDef {
            id: id("extern", "def"),
            tparams: vec![],
            params: vec![param(ast::ParamKind::Exp(bool_typ()))],
            plain_typ: bool_typ(),
            hints: vec![hint.clone()],
        })),
        definition(ast::DefKind::BuiltinDec(ast::BuiltinDecDef {
            id: id("builtin", "def"),
            tparams: vec![],
            params: vec![],
            plain_typ: bool_typ(),
            hints: vec![hint.clone()],
        })),
        definition(ast::DefKind::TableDec(ast::TableDecDef {
            id: id("table", "def"),
            params: vec![],
            plain_typ: bool_typ(),
            hints: vec![hint.clone()],
        })),
        definition(ast::DefKind::FuncDec(ast::FuncDecDef {
            id: id("declared", "def"),
            tparams: vec![],
            params: vec![],
            plain_typ: bool_typ(),
            hints: vec![hint.clone()],
        })),
        definition(ast::DefKind::TableDef(ast::TableDef {
            id: id("rows", "def"),
            rows: vec![Spanned::new(
                (
                    exp(ExpKind::Var(id("pattern", "row")), "row"),
                    exp(ExpKind::Var(id("body", "row")), "row"),
                ),
                span("row"),
            )],
        })),
        definition(ast::DefKind::FuncDef(ast::FuncDef {
            id: id("defined", "def"),
            tparams: vec![],
            args: vec![],
            exp: exp(ExpKind::Var(id("body", "def")), "def"),
            prems: vec![prem(ast::PremKind::Else)],
        })),
        definition(ast::DefKind::Sep),
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
        print::string_of_exp(&exp(ExpKind::Text(escaped.into()), "text")),
        "\"\\\"\\\\'\\n\\r\\t\\b\\012\\001\\195\\169\""
    );
    assert_eq!(
        print::string_of_exp(&exp(ExpKind::Latex(escaped.into()), "latex")),
        "latex(\\\"\\\\'\\n\\r\\t\\b\\012\\001\\195\\169)"
    );
    assert_eq!(
        print::string_of_unop(ast::UnOp::Num(p4spec_rust::lang::xl::num::UnOp::Minus)),
        "-"
    );
    assert_eq!(
        print::string_of_binop(ast::BinOp::Bool(p4spec_rust::lang::xl::bool::BinOp::Equiv,)),
        "<=>"
    );
    assert_eq!(
        print::string_of_cmpop(ast::CmpOp::Bool(p4spec_rust::lang::xl::bool::CmpOp::Ne)),
        "=/="
    );
    let atom_type = Spanned::new(ast::NotTypKind::Atom(atom("A")), span("type"));
    assert_eq!(
        print::string_of_not_typs(", ", &[atom_type.clone(), atom_type]),
        "A, A"
    );
    let row = Spanned::new(
        (exp(ExpKind::Eps, "row"), exp(ExpKind::Eps, "row")),
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
            exp(ExpKind::Eps, "rule"),
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
    let var = |name| exp(ExpKind::Var(id(name, "expr")), "expr");
    let expressions = [
        (
            exp(
                ExpKind::Num(ast::NumOp::Hex, ast::Num::Nat(15.into())),
                "expr",
            ),
            "0xF",
        ),
        (exp(ExpKind::Arith(Box::new(var("x"))), "expr"), "$(x)"),
        (
            exp(ExpKind::List(vec![var("x"), var("y")]), "expr"),
            "[x, y]",
        ),
        (
            exp(
                ExpKind::Cons(Box::new(var("x")), Box::new(var("xs"))),
                "expr",
            ),
            "x :: xs",
        ),
        (
            exp(ExpKind::Cat(Box::new(var("x")), Box::new(var("y"))), "expr"),
            "x ++ y",
        ),
        (
            exp(
                ExpKind::Slice(Box::new(var("x")), Box::new(var("i")), Box::new(var("j"))),
                "expr",
            ),
            "x[i : j]",
        ),
        (
            exp(ExpKind::Str(vec![(atom("field"), var("x"))]), "expr"),
            "{field x}",
        ),
        (
            exp(
                ExpKind::Infix(Box::new(var("x")), atom("'++'"), Box::new(var("y"))),
                "expr",
            ),
            "x '++' y",
        ),
        (
            exp(
                ExpKind::Brack(atom("`("), Box::new(var("x")), atom("`)")),
                "expr",
            ),
            "``(x`)",
        ),
        (
            exp(
                ExpKind::Fuse(Box::new(var("x")), Box::new(var("y"))),
                "expr",
            ),
            "x#y",
        ),
    ];
    for (expression, expected) in expressions {
        assert_eq!(print::string_of_exp(&expression), expected);
    }

    let root = Spanned::new(ast::PathKind::Root, span("path"));
    let paths = [
        (root.clone(), ""),
        (
            Spanned::new(
                ast::PathKind::Idx(Box::new(root.clone()), Box::new(var("i"))),
                span("path"),
            ),
            "[i]",
        ),
        (
            Spanned::new(
                ast::PathKind::Slice(
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
                ast::PathKind::Dot(Box::new(root), atom("field")),
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
            plain(ast::PlainTypKind::Num(p4spec_rust::lang::xl::num::Typ::Int)),
            "int",
        ),
        (
            plain(ast::PlainTypKind::Paren(Box::new(bool_typ()))),
            "(bool)",
        ),
        (
            plain(ast::PlainTypKind::Tuple(vec![bool_typ(), bool_typ()])),
            "(bool, bool)",
        ),
        (
            plain(ast::PlainTypKind::Iter(
                Box::new(bool_typ()),
                ast::Iter::List,
            )),
            "bool*",
        ),
    ];
    for (typ, expected) in types {
        assert_eq!(print::string_of_plain_typ(&typ), expected);
    }
    let notation = [
        (
            Spanned::new(
                ast::NotTypKind::Seq(vec![
                    ast::Typ::Plain(bool_typ()),
                    ast::Typ::Plain(bool_typ()),
                ]),
                span("type"),
            ),
            "bool bool",
        ),
        (
            Spanned::new(
                ast::NotTypKind::Infix(
                    Box::new(ast::Typ::Plain(bool_typ())),
                    atom("'~'"),
                    Box::new(ast::Typ::Plain(bool_typ())),
                ),
                span("type"),
            ),
            "bool '~' bool",
        ),
        (
            Spanned::new(
                ast::NotTypKind::Brack(
                    atom("`["),
                    Box::new(ast::Typ::Plain(bool_typ())),
                    atom("`]"),
                ),
                span("type"),
            ),
            "``[bool`]",
        ),
    ];
    for (typ, expected) in notation {
        assert_eq!(print::string_of_not_typ(&typ), expected);
    }
    let def_types = [
        (
            Spanned::new(ast::DefTypKind::Plain(bool_typ()), span("type")),
            "bool",
        ),
        (
            Spanned::new(
                ast::DefTypKind::Variant(vec![(ast::Typ::Plain(bool_typ()), vec![])]),
                span("type"),
            ),
            "\n   | bool",
        ),
    ];
    for (typ, expected) in def_types {
        assert_eq!(print::string_of_def_typ(&typ), expected);
    }

    let premises = [
        (
            prem(ast::PremKind::Var(ast::VarPrem {
                id: id("x", "prem"),
                plain_typ: bool_typ(),
            })),
            "x : bool",
        ),
        (
            prem(ast::PremKind::Rule(ast::RulePrem {
                id: id("r", "prem"),
                exp: var("x"),
            })),
            "r: x",
        ),
        (
            prem(ast::PremKind::RuleNot(ast::RuleNotPrem {
                id: id("r", "prem"),
                exp: var("x"),
            })),
            "r:/ x",
        ),
        (
            prem(ast::PremKind::Debug(ast::DebugPrem { exp: var("x") })),
            "debug x",
        ),
    ];
    for (premise, expected) in premises {
        assert_eq!(print::string_of_prem(&premise), expected);
    }
    let rule = Spanned::new(
        (id("r", "rule"), id("g", "rule"), var("x"), vec![]),
        span("rule"),
    );
    assert_eq!(
        print::string_of_def(&definition(ast::DefKind::RuleGroup(ast::RuleGroupDef {
            relid: id("r", "def"),
            groupid: id("g", "def"),
            rules: vec![rule],
        }))),
        "rulegroup r/g:\n  rule r/g:\n  x"
    );
}
