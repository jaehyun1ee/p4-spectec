use p4spec_rust::{
    domain::{
        atom::Atom,
        mixfix::Mixfix,
        source::{Region, Spanned},
    },
    lang::{el, il, sl},
};

fn span(name: &str) -> Region {
    Region::for_file(name)
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
    il::ast::Exp::new(
        il::ast::ExpKind::VarE(id(name)),
        il::ast::TypKind::BoolT,
        span(name),
    )
}

fn text(value: &str) -> il::ast::Exp {
    il::ast::Exp::new(
        il::ast::ExpKind::TextE(value.to_owned()),
        il::ast::TypKind::TextT,
        span("text"),
    )
}

fn notation() -> il::ast::NotTyp {
    Spanned::new(
        Mixfix::Seq(vec![
            Mixfix::Atom(atom("eval")),
            Mixfix::Arg(typ(il::ast::TypKind::BoolT)),
            Mixfix::Atom(atom("=>")),
            Mixfix::Arg(typ(il::ast::TypKind::TextT)),
        ]),
        span("notation"),
    )
}

fn instr(kind: sl::ast::InstrKind, iid: i64) -> sl::ast::Instr {
    sl::ast::Instr::new(kind, iid, span("instruction"))
}

fn hint(source: &str) -> el::ast::Hint {
    el::ast::Hint {
        hintid: id("metadata"),
        hintexp: Spanned::new(el::ast::ExpKind::TextE(source.to_owned()), span(source)),
    }
}

fn composite_spec(metadata: &str) -> sl::ast::Spec {
    let signature = (notation(), vec![0]);
    let parameter = Spanned::new(
        sl::ast::ParamKind::ExpP(typ(il::ast::TypKind::BoolT), variable("default")),
        span("parameter"),
    );
    let hints = vec![hint(metadata)];

    vec![
        Spanned::new(
            sl::ast::DefKind::ExternTypD(id("External"), hints.clone()),
            span(metadata),
        ),
        Spanned::new(
            sl::ast::DefKind::VarD(id("state"), typ(il::ast::TypKind::BoolT), hints.clone()),
            span(metadata),
        ),
        Spanned::new(
            sl::ast::DefKind::RelD((
                id("Evaluate"),
                signature.clone(),
                vec![variable("input")],
                vec![instr(
                    sl::ast::InstrKind::ResultI(signature.clone(), vec![text("line\n\"\\")]),
                    7,
                )],
                Some(vec![instr(
                    sl::ast::InstrKind::ReturnI(variable("fallback")),
                    8,
                )]),
                hints.clone(),
            )),
            span(metadata),
        ),
        Spanned::new(
            sl::ast::DefKind::ExternDecD((
                id("external"),
                Vec::new(),
                vec![parameter.clone()],
                typ(il::ast::TypKind::BoolT),
                hints.clone(),
            )),
            span(metadata),
        ),
        Spanned::new(
            sl::ast::DefKind::TableDecD((
                id("lookup"),
                vec![parameter.clone()],
                typ(il::ast::TypKind::BoolT),
                vec![(
                    vec![variable("key")],
                    text("row\tvalue"),
                    vec![instr(
                        sl::ast::InstrKind::ReturnI(variable("row-result")),
                        9,
                    )],
                )],
                hints.clone(),
            )),
            span(metadata),
        ),
        Spanned::new(
            sl::ast::DefKind::FuncDecD((
                id("run"),
                Vec::new(),
                vec![parameter],
                typ(il::ast::TypKind::BoolT),
                vec![instr(sl::ast::InstrKind::ReturnI(text("done")), 10)],
                None,
                hints,
            )),
            span(metadata),
        ),
    ]
}

#[test]
fn composite_spec_prints_in_order_with_escaping_and_instruction_levels() {
    assert_eq!(
        sl::print::string_of_spec(&composite_spec("source-a")),
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
        sl::print::string_of_spec(&composite_spec("source-a")),
        sl::print::string_of_spec(&composite_spec("source-b"))
    );
}

#[test]
fn dangling_branches_render_the_instruction_identifier() {
    let branch = instr(
        sl::ast::InstrKind::IfI(
            variable("condition"),
            Vec::new(),
            vec![instr(sl::ast::InstrKind::ReturnI(variable("value")), 2)],
            true,
        ),
        42,
    );

    assert_eq!(
        sl::print::string_of_instr_with(&branch, false, 0, 1),
        "1. If (condition), then\n\n  1. Return value\n\n1. Else Dangling#42"
    );
}
