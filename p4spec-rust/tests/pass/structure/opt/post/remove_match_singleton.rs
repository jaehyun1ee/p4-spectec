use super::*;
use crate::{
    pass::structure::opt::post::remove_match_singleton::apply,
    runtime::{envs::algo::TDEnv, typdef::TypeDef},
};
fn variant(tdenv: &mut TDEnv, text: &str, texts: &[&str]) -> Typ {
    let typ = crate::phrase!(node: TypKind::Var(id(text), vec![]), span: span(2));
    let typcases = texts
        .iter()
        .map(|text_case| {
            let mixop = crate::frontend::parse::parse_mixop(text_case).unwrap();
            (
                crate::phrase!(node: mixop.map(|_| typ.clone()), span: span(3)),
                crate::phrase!(node: (id(text), vec![]), span: span(4)),
                vec![],
            )
        })
        .collect();
    tdenv.insert(
        id(text),
        TypeDef::Defined(
            vec![],
            Box::new(crate::phrase!(node: DefTypKind::Variant(typcases), span: span(5))),
        ),
    );
    typ
}
fn matching(typ: &Typ, block: Block) -> Instr {
    let exp =
        crate::note_phrase!(node: ExpKind::Var(id("value")), note: typ.node.clone(), span: span(7));
    let exp = crate::note_phrase!(node: ExpKind::Match(Box::new(exp), Pattern::Case(Box::new(crate::frontend::parse::parse_mixop("A").unwrap()))), note: TypKind::Bool, span: span(8));
    instr(InstrKind::If(IfInstr {
        exp,
        iter_exps: vec![(Iter::Opt, vec![])],
        block,
    }))
}
#[test]
fn test_singleton_alias_removes_match_but_multi_variant_and_primitive_do_not() {
    let mut tdenv = TDEnv::new();
    let typ_single = variant(&mut tdenv, "Single", &["A"]);
    let typ_multi = variant(&mut tdenv, "Multi", &["A", "B"]);
    tdenv.insert(
        id("Alias"),
        TypeDef::Defined(
            vec![],
            Box::new(crate::phrase!(node: DefTypKind::Plain(typ_single.clone()), span: span(9))),
        ),
    );
    let typ_alias = crate::phrase!(node: TypKind::Var(id("Alias"), vec![]), span: span(9));
    assert_eq!(
        apply(
            &tdenv,
            vec![matching(
                &typ_alias,
                vec![matching(&typ_single, vec![ret("result")])]
            )]
        )
        .unwrap(),
        vec![ret("result")]
    );
    for typ in [
        typ_multi,
        crate::phrase!(node: TypKind::Bool, span: span(10)),
    ] {
        let instr_match = matching(&typ, vec![ret("result")]);
        assert_eq!(
            apply(&tdenv, vec![instr_match.clone()]).unwrap(),
            vec![instr_match]
        );
    }
}
#[test]
fn test_nested_containers_rewrite_but_debug_remains_opaque() {
    let mut tdenv = TDEnv::new();
    let typ = variant(&mut tdenv, "Single", &["A"]);
    let wrap = |instr_inner: Instr| {
        vec![
            instr(InstrKind::Hold(HoldInstr {
                id: id("R"),
                not_exp: Mixfix::Arg(variable("arg")),
                iter_exps: vec![],
                block_hold: vec![instr_inner.clone()],
                block_not_hold: vec![instr_inner.clone()],
            })),
            instr(InstrKind::Case(CaseInstr {
                exp: variable("cond"),
                cases: vec![Case {
                    guard: Guard::Bool(true),
                    block: vec![instr_inner.clone()],
                }],
                total: false,
            })),
            instr(InstrKind::Group(GroupInstr {
                id: id("G"),
                rel_signature: signature(),
                exps: vec![],
                block: vec![instr_inner.clone()],
            })),
            binding(literal(), vec![instr_inner.clone()]),
            rule(vec![instr_inner], InputHint::new(vec![0])),
        ]
    };
    assert_eq!(
        apply(&tdenv, wrap(matching(&typ, vec![ret("result")]))).unwrap(),
        wrap(ret("result"))
    );
    let instr_debug = instr(InstrKind::Debug(DebugInstr {
        exp: variable("debug"),
        instr: Box::new(matching(&typ, vec![ret("result")])),
    }));
    assert_eq!(
        apply(&tdenv, vec![instr_debug.clone()]).unwrap(),
        vec![instr_debug]
    );
}
