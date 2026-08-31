use super::*;

#[test]
fn test_printer_preserves_el_delimiters_precedence_hints_and_definition_separators() {
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
    assert_eq!(Print::to_string(&nested), "(a + b) * c?");
    assert_eq!(
        Print::to_string(&exp(
            ExpKind::Call(
                id("f", "call"),
                vec![plain(ast::PlainTypKind::Text)],
                vec![p4spec_rust::phrase! {
                    node: ast::ArgKind::Def(id("g", "call")),
                    span: span("call"),
                }]
            ),
            "call"
        )),
        "$f<text>($g)"
    );
    assert_eq!(
        Print::to_string(&prem(ast::PremKind::Iter(ast::IterPrem {
            prem: Box::new(prem(ast::PremKind::If(ast::IfPrem {
                exp: exp(ExpKind::Var(id("ready", "prem")), "prem"),
            }))),
            iter: ast::Iter::List,
        }))),
        "(if ready)*"
    );

    let not_typ = p4spec_rust::phrase! {
        node: ast::NotTypKind::Atom(atom("TERM")),
        span: span("type"),
    };
    let definitions = vec![
        definition(ast::DefKind::ExternSyntax(ast::ExternSyntaxDef {
            id: id("Syntax", "def"),
            hints: vec![hint.clone()],
        })),
        definition(ast::DefKind::Syntax(ast::SyntaxDef {
            entries: vec![ast::SyntaxDefEntry {
                id: id("Pair", "def"),
                tparams: vec![p4spec_rust::phrase! {
                    node: "T".to_owned(),
                    span: span("def"),
                }],
            }],
        })),
        definition(ast::DefKind::Typ(ast::TypDef {
            id: id("Record", "def"),
            tparams: vec![],
            def_typ: p4spec_rust::phrase! {
                node: ast::DefTypKind::Struct(vec![(atom("field"), bool_typ(), vec![hint.clone()])]),
                span: span("def"),
            },
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
            rows: vec![p4spec_rust::phrase! { node: (
                exp(ExpKind::Var(id("pattern", "row")), "row"),
                exp(ExpKind::Var(id("body", "row")), "row"),
            ), span: span("row") }],
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
        Print::to_string(&definitions),
        "extern syntax Syntax\nsyntax Pair<T>\nsyntax Record = {field bool}\nvar value : bool\nextern relation external: TERM\nrelation internal: TERM\nextern dec $extern(bool) : bool\nbuiltin dec $builtin : bool\ntbl dec $table : bool\ndec $declared : bool\ntbl def $rows =\n  pattern => body\ndef $defined = body\n -- otherwise\n\n\n\n"
    );
}
#[test]
fn test_printer_matches_ocaml_byte_escaping_and_public_collection_helpers() {
    let escaped = "\"\\'\n\r\t\x08\x0c\x01é";
    assert_eq!(
        Print::to_string(&exp(ExpKind::Text(escaped.into()), "text")),
        "\"\\\"\\\\'\\n\\r\\t\\b\\012\\001\\195\\169\""
    );
    assert_eq!(
        Print::to_string(&exp(ExpKind::Latex(escaped.into()), "latex")),
        "latex(\\\"\\\\'\\n\\r\\t\\b\\012\\001\\195\\169)"
    );
    assert_eq!(
        Print::to_string(&ast::UnOp::Num(p4spec_rust::lang::xl::num::UnOp::Minus)),
        "-"
    );
    assert_eq!(
        Print::to_string(&ast::BinOp::Bool(p4spec_rust::lang::xl::bool::BinOp::Equiv,)),
        "<=>"
    );
    assert_eq!(
        Print::to_string(&ast::CmpOp::Bool(p4spec_rust::lang::xl::bool::CmpOp::Ne)),
        "=/="
    );
    let atom_type = p4spec_rust::phrase! {
        node: ast::NotTypKind::Atom(atom("A")),
        span: span("type"),
    };
    assert_eq!(
        Print::to_string(&[atom_type.clone(), atom_type][..]),
        "A, A"
    );
    let row = p4spec_rust::phrase! {
        node: (exp(ExpKind::Eps, "row"), exp(ExpKind::Eps, "row")),
        span: span("row"),
    };
    assert_eq!(
        Print::to_string(&[row.clone(), row][..]),
        "eps => eps\n  | eps => eps"
    );
    let rule = p4spec_rust::phrase! { node: (
        id("r", "rule"),
        id("", "rule"),
        exp(ExpKind::Eps, "rule"),
        vec![],
    ), span: span("rule") };
    assert_eq!(
        Print::to_string(&[rule.clone(), rule][..]),
        "rule r:\n  eps\nrule r:\n  eps"
    );
}
