use p4spec_rust::lang::{
    common::{
        notation::{atom::Atom, mixfix::Mixfix},
        source::{Position, Span, Spanned},
    },
    el,
    hints::input::InputHint,
    il, sl,
    traits::print::Print,
};

fn span(name: &str) -> Span {
    Span::new(Position::new(name, 0, 0), Position::new(name, 0, 0))
}

fn id(name: &str) -> il::ast::Id {
    Spanned::new(name.to_owned(), span(name))
}

fn atom(name: &str) -> il::ast::Atom {
    Spanned::new(Atom::Keyword(name.to_owned()), span(name))
}

fn typ(kind: il::ast::TypKind) -> il::ast::Typ {
    Spanned::new(kind, span("type"))
}

fn variable(name: &str) -> il::ast::Exp {
    il::ast::exp(
        il::ast::ExpKind::Var(id(name)),
        il::ast::TypKind::Bool,
        span(name),
    )
}

fn text(value: &str) -> il::ast::Exp {
    il::ast::exp(
        il::ast::ExpKind::Text(value.to_owned()),
        il::ast::TypKind::Text,
        span("text"),
    )
}

fn notation() -> il::ast::NotTyp {
    Spanned::new(
        Mixfix::Seq(vec![
            Mixfix::Atom(atom("eval")),
            Mixfix::Arg(typ(il::ast::TypKind::Bool)),
            Mixfix::Atom(atom("=>")),
            Mixfix::Arg(typ(il::ast::TypKind::Text)),
        ]),
        span("notation"),
    )
}

fn instr(kind: sl::ast::InstrKind, iid: i64) -> sl::ast::Instr {
    sl::ast::instr(kind, iid, span("instruction"))
}

fn hint(source: &str) -> el::ast::Hint {
    (
        id("metadata"),
        Spanned::new(el::ast::ExpKind::Text(source.to_owned()), span(source)),
    )
}

fn composite_spec(metadata: &str) -> sl::ast::Spec {
    let signature = sl::ast::RelSignature {
        not_typ: notation(),
        input_hint: InputHint::new(vec![0]),
    };
    let parameter = Spanned::new(
        sl::ast::ParamKind::Exp(typ(il::ast::TypKind::Bool), Box::new(variable("default"))),
        span("parameter"),
    );
    let hints = vec![hint(metadata)];

    vec![
        Spanned::new(
            sl::ast::DefKind::ExternTyp(sl::ast::ExternTypDef {
                id: id("External"),
                hints: hints.clone(),
            }),
            span(metadata),
        ),
        Spanned::new(
            sl::ast::DefKind::Var(sl::ast::VarDef {
                id: id("state"),
                typ: typ(il::ast::TypKind::Bool),
                hints: hints.clone(),
            }),
            span(metadata),
        ),
        Spanned::new(
            sl::ast::DefKind::Rel(sl::ast::Rel {
                id: id("Evaluate"),
                rel_signature: signature.clone(),
                exps_input: vec![variable("input")],
                block: vec![instr(
                    sl::ast::InstrKind::Result(sl::ast::ResultInstr {
                        rel_signature: signature.clone(),
                        exps: vec![text("line\n\"\\")],
                    }),
                    7,
                )],
                else_block: Some(vec![instr(
                    sl::ast::InstrKind::Return(sl::ast::ReturnInstr {
                        exp: variable("fallback"),
                    }),
                    8,
                )]),
                hints: hints.clone(),
            }),
            span(metadata),
        ),
        Spanned::new(
            sl::ast::DefKind::ExternDec(sl::ast::ExternFunc {
                id: id("external"),
                tparams: Vec::new(),
                params: vec![parameter.clone()],
                typ: typ(il::ast::TypKind::Bool),
                hints: hints.clone(),
            }),
            span(metadata),
        ),
        Spanned::new(
            sl::ast::DefKind::TableDec(sl::ast::TableFunc {
                id: id("lookup"),
                params: vec![parameter.clone()],
                typ: typ(il::ast::TypKind::Bool),
                table_rows: vec![sl::ast::TableRow {
                    exps_input: vec![variable("key")],
                    exp: text("row\tvalue"),
                    block: vec![instr(
                        sl::ast::InstrKind::Return(sl::ast::ReturnInstr {
                            exp: variable("row-result"),
                        }),
                        9,
                    )],
                }],
                hints: hints.clone(),
            }),
            span(metadata),
        ),
        Spanned::new(
            sl::ast::DefKind::FuncDec(sl::ast::DefinedFunc {
                id: id("run"),
                tparams: Vec::new(),
                params: vec![parameter],
                typ: typ(il::ast::TypKind::Bool),
                block: vec![instr(
                    sl::ast::InstrKind::Return(sl::ast::ReturnInstr { exp: text("done") }),
                    10,
                )],
                else_block: None,
                hints,
            }),
            span(metadata),
        ),
    ]
}

#[test]
fn composite_spec_prints_in_order_with_escaping_and_instruction_levels() {
    assert_eq!(
        Print::render(&composite_spec("source-a")),
        concat!(
            "extern syntax External\n\n",
            "var state : bool\n\n",
            "relation Evaluate: eval input => %\n\n",
            "1. Result in: eval % => \"line\\n\\\"\\\\\"\n\n",
            "2. Otherwise,\n\n",
            "  1. Return fallback\n\n",
            "extern def $external(default)\n\n",
            "tbl def $lookup(default)\n=\n\n",
            "  Row : key -> \"row\\tvalue\":\n\n",
            "    1. Return row-result\n\n",
            "def $run(default)\n\n",
            "1. Return \"done\"",
        )
    );
}

#[test]
fn specification_printer_omits_source_and_hint_metadata() {
    assert_eq!(
        Print::render(&composite_spec("source-a")),
        Print::render(&composite_spec("source-b"))
    );
}

#[test]
fn dangling_branches_render_the_instruction_identifier() {
    let branch = instr(
        sl::ast::InstrKind::If(sl::ast::IfInstr {
            exp: variable("condition"),
            iter_exps: Vec::new(),
            block: vec![instr(
                sl::ast::InstrKind::Return(sl::ast::ReturnInstr {
                    exp: variable("value"),
                }),
                2,
            )],
            dangle: true,
        }),
        42,
    );

    assert_eq!(
        sl::print::render_instr_with(&branch, false, 0, 1),
        "1. If (condition), then\n\n  1. Return value\n\n1. Else Dangling#42"
    );
}
