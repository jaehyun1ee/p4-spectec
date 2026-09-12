use super::*;
use crate::{
    pass::structure::{StructureErrorKind, totalize::*},
    runtime::{envs::algo::TDEnv, typdef::TypeDef},
};
fn variant(tdenv: &mut TDEnv, text: &str, texts: &[&str]) -> Typ {
    let typ = crate::phrase!(node: TypKind::Var(id(text), vec![]), span: span(2));
    let typcases = texts
        .iter()
        .map(|text_case| {
            let mixop = crate::frontend::parse::parse_mixop(text_case).unwrap();
            let not_typ = crate::phrase!(node: mixop.map(|_| typ.clone()), span: span(3));
            (
                not_typ,
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
fn pattern(text: &str) -> Guard {
    Guard::Match(Pattern::Case(Box::new(
        crate::frontend::parse::parse_mixop(text).unwrap(),
    )))
}
fn case(typ: &Typ, guards: Vec<Guard>, total: bool) -> ast_ol::Instr {
    let exp =
        crate::note_phrase!(node: ExpKind::Var(id("value")), note: typ.node.clone(), span: span(7));
    instr(ast_ol::InstrKind::Case(ast_ol::CaseInstr {
        exp,
        cases: guards
            .into_iter()
            .enumerate()
            .map(|(idx, guard)| ast_ol::Case {
                guard,
                block: vec![ret(&idx.to_string())],
            })
            .collect(),
        total,
    }))
}
fn is_total(block: &ast_ol::Block) -> bool {
    let ast_ol::InstrKind::Case(instr_case) = &block[0].node else {
        panic!()
    };
    instr_case.total
}
#[test]
fn test_variant_coverage_duplicates_order_and_alias() {
    let mut tdenv = TDEnv::new();
    let typ = variant(&mut tdenv, "Choice", &["A", "B"]);
    tdenv.insert(
        id("Alias"),
        TypeDef::Defined(
            vec![],
            Box::new(crate::phrase!(node: DefTypKind::Plain(typ.clone()), span: span(8))),
        ),
    );
    let typ_alias = crate::phrase!(node: TypKind::Var(id("Alias"), vec![]), span: span(8));
    for (guards, total_expect) in [
        (vec![pattern("B"), pattern("A"), pattern("A")], true),
        (vec![pattern("A"), pattern("A")], false),
        (vec![pattern("A"), pattern("B"), pattern("C")], false),
    ] {
        let instr_case = case(&typ_alias, guards, !total_expect);
        let mut instr_expect = instr_case.clone();
        let ast_ol::InstrKind::Case(instr_case_expect) = &mut instr_expect.node else {
            panic!()
        };
        instr_case_expect.total = total_expect;
        let block = totalize_without_else(&tdenv, vec![instr_case]).unwrap();
        assert_eq!(block, vec![instr_expect]);
    }
}
#[test]
fn test_subtype_pattern_coverage_and_nonvariant_guard_short_circuit() {
    let mut tdenv = TDEnv::new();
    let typ = variant(&mut tdenv, "Choice", &["A", "B"]);
    let typ_a = variant(&mut tdenv, "Single", &["A"]);
    let guard = Guard::Sub(typ_a, Box::new(crate::lang::il::ast::Subcheck::Skip));
    let block =
        totalize_without_else(&tdenv, vec![case(&typ, vec![guard, pattern("B")], false)]).unwrap();
    assert!(is_total(&block));
    let typ_bool = crate::phrase!(node: TypKind::Bool, span: span(12));
    for guard in [
        Guard::Bool(true),
        Guard::Match(Pattern::Opt(crate::lang::il::ast::OptPattern::Some)),
        Guard::Mem(variable("set")),
    ] {
        let instr_case = case(
            &typ_bool,
            vec![
                guard,
                Guard::Sub(
                    typ_bool.clone(),
                    Box::new(crate::lang::il::ast::Subcheck::Skip),
                ),
            ],
            true,
        );
        assert_eq!(
            totalize_without_else(&tdenv, vec![instr_case.clone()]).unwrap(),
            vec![instr_case]
        );
    }
}
#[test]
fn test_nonvariant_failures_are_located_and_debug_is_not_totalized() {
    let typ = crate::phrase!(node: TypKind::Bool, span: span(12));
    for (guards, span_expect) in [
        (vec![pattern("A")], span(7)),
        (
            vec![Guard::Sub(
                typ.clone(),
                Box::new(crate::lang::il::ast::Subcheck::Skip),
            )],
            span(12),
        ),
    ] {
        let error =
            totalize_without_else(&TDEnv::new(), vec![case(&typ, guards, false)]).unwrap_err();
        assert_eq!(error.kind, StructureErrorKind::NonVariantTotalization);
        assert_eq!(error.span, span_expect);
    }
    let instr_debug = instr(ast_ol::InstrKind::Debug(ast_ol::DebugInstr {
        exp: variable("debug"),
        instr: Box::new(case(&typ, vec![pattern("A")], false)),
    }));
    let blocks = totalize(
        &TDEnv::new(),
        vec![instr_debug.clone()],
        Some(vec![instr_debug.clone()]),
    )
    .unwrap();
    assert_eq!(blocks.block, vec![instr_debug.clone()]);
    assert_eq!(blocks.block_else, Some(vec![instr_debug]));
}

#[test]
fn test_totalization_descends_all_owning_blocks_and_else() {
    let mut tdenv = TDEnv::new();
    let typ = variant(&mut tdenv, "Choice", &["A"]);
    let instr_case = case(&typ, vec![pattern("A")], false);
    let mut instr_expect = instr_case.clone();
    let ast_ol::InstrKind::Case(instr_case_expect) = &mut instr_expect.node else {
        panic!()
    };
    instr_case_expect.total = true;
    let wrap = |instr_inner: ast_ol::Instr| {
        vec![
            instr(ast_ol::InstrKind::If(ast_ol::IfInstr {
                exp: variable("cond"),
                iter_exps: vec![],
                block: vec![instr_inner.clone()],
            })),
            instr(ast_ol::InstrKind::Hold(ast_ol::HoldInstr {
                id: id("R"),
                not_exp: Mixfix::Arg(variable("arg")),
                iter_exps: vec![],
                block_hold: vec![instr_inner.clone()],
                block_not_hold: vec![instr_inner.clone()],
            })),
            instr(ast_ol::InstrKind::Group(ast_ol::GroupInstr {
                id: id("group"),
                rel_signature: signature(),
                exps: vec![variable("input")],
                block: vec![instr_inner.clone()],
            })),
            instr(ast_ol::InstrKind::Let(ast_ol::LetInstr {
                exp_l: variable("bound"),
                exp_r: variable("input"),
                iter_instrs: vec![],
                block: vec![instr_inner.clone()],
            })),
            instr(ast_ol::InstrKind::Rule(ast_ol::RuleInstr {
                id: id("R"),
                not_exp: Mixfix::Arg(variable("arg")),
                input_hint: InputHint::new(vec![0]),
                iter_instrs: vec![],
                block: vec![instr_inner.clone()],
            })),
            instr(ast_ol::InstrKind::Case(ast_ol::CaseInstr {
                exp: variable("bool"),
                cases: vec![ast_ol::Case {
                    guard: Guard::Bool(true),
                    block: vec![instr_inner],
                }],
                total: false,
            })),
        ]
    };
    let blocks = totalize(&tdenv, wrap(instr_case.clone()), Some(wrap(instr_case))).unwrap();
    assert_eq!(blocks.block, wrap(instr_expect.clone()));
    assert_eq!(blocks.block_else, Some(wrap(instr_expect)));
    assert_eq!(
        totalize(&tdenv, vec![], Some(vec![])).unwrap().block_else,
        Some(vec![])
    );
    assert!(totalize(&tdenv, vec![], None).unwrap().block_else.is_none());
}
