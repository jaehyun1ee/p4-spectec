use super::*;
use crate::pass::structure::{StructureErrorKind, dangle::*};
fn conditional(text: &str, block: ast_ol::Block) -> ast_ol::Instr {
    instr(ast_ol::InstrKind::If(ast_ol::IfInstr {
        exp: variable(text),
        iter_exps: vec![],
        block,
    }))
}
fn hold(block_hold: ast_ol::Block, block_not_hold: ast_ol::Block) -> ast_ol::Instr {
    instr(ast_ol::InstrKind::Hold(ast_ol::HoldInstr {
        id: id("R"),
        not_exp: Mixfix::Arg(variable("arg")),
        iter_exps: vec![],
        block_hold,
        block_not_hold,
    }))
}
fn block() -> ast_ol::Block {
    vec![
        conditional(
            "j_nonnegative",
            vec![conditional("i_nonnegative", vec![ret("positive")])],
        ),
        conditional(
            "i_negative",
            vec![conditional("j_nonnegative", vec![ret("negative")])],
        ),
        hold(vec![ret("hold")], vec![]),
        hold(vec![], vec![ret("not_hold")]),
        hold(vec![ret("hold")], vec![ret("not_hold")]),
        instr(ast_ol::InstrKind::Case(ast_ol::CaseInstr {
            exp: variable("case"),
            cases: vec![],
            total: false,
        })),
        instr(ast_ol::InstrKind::Case(ast_ol::CaseInstr {
            exp: variable("total"),
            cases: vec![],
            total: true,
        })),
        instr(ast_ol::InstrKind::Debug(ast_ol::DebugInstr {
            exp: variable("debug"),
            instr: Box::new(conditional("inner", vec![ret("done")])),
        })),
    ]
}
fn check(block_sl: &Block, dangle: bool) {
    assert_eq!(block_sl.len(), 8);
    for (instr_sl, text_outer, text_inner) in [
        (&block_sl[0], "j_nonnegative", "i_nonnegative"),
        (&block_sl[1], "i_negative", "j_nonnegative"),
    ] {
        let InstrKind::If(instr_if_sl) = &instr_sl.node else {
            panic!()
        };
        assert_eq!(instr_if_sl.dangle, dangle);
        assert_eq!(instr_if_sl.exp, variable(text_outer));
        let InstrKind::If(instr_inner_sl) = &instr_if_sl.block[0].node else {
            panic!()
        };
        assert_eq!(instr_inner_sl.dangle, dangle);
        assert_eq!(instr_inner_sl.exp, variable(text_inner));
        assert_eq!(instr_sl.span, span(1));
    }
    assert!(
        matches!(&block_sl[2].node, InstrKind::Hold(HoldInstr {hold_case: HoldCase::Hold(_, flag), ..}) if *flag == dangle)
    );
    assert!(
        matches!(&block_sl[3].node, InstrKind::Hold(HoldInstr {hold_case: HoldCase::NotHold(_, flag), ..}) if *flag == dangle)
    );
    assert!(matches!(
        &block_sl[4].node,
        InstrKind::Hold(HoldInstr {
            hold_case: HoldCase::Both(_, _),
            ..
        })
    ));
    assert!(
        matches!(&block_sl[5].node, InstrKind::Case(CaseInstr {dangle: flag, ..}) if *flag == dangle)
    );
    assert!(matches!(
        &block_sl[6].node,
        InstrKind::Case(CaseInstr { dangle: false, .. })
    ));
    let InstrKind::Debug(instr_debug_sl) = &block_sl[7].node else {
        panic!()
    };
    assert_eq!(instr_debug_sl.exp, variable("debug"));
    assert!(
        matches!(&instr_debug_sl.instr.node, InstrKind::If(IfInstr {dangle: flag, ..}) if *flag == dangle)
    );
}
#[test]
fn test_fallthrough_modes_and_reordered_nested_conditions() {
    let blocks = instrument(block(), None).unwrap();
    check(&blocks.block, true);
    assert!(blocks.block_else.is_none());
    let blocks = instrument(block(), Some(vec![])).unwrap();
    check(&blocks.block, false);
    assert_eq!(blocks.block_else, Some(vec![]));
    let blocks = instrument(block(), Some(block())).unwrap();
    check(&blocks.block, false);
    check(&blocks.block_else.unwrap(), false);
    check(&instrument_without_else(block()).unwrap(), false);
}
#[test]
fn test_empty_hold_nested_in_debug_has_owning_span() {
    let mut instr_hold = hold(vec![], vec![]);
    instr_hold.span = span(42);
    let instr_debug = instr(ast_ol::InstrKind::Debug(ast_ol::DebugInstr {
        exp: variable("debug"),
        instr: Box::new(instr_hold),
    }));
    for block_else in [None, Some(vec![])] {
        let error = instrument(vec![instr_debug.clone()], block_else).unwrap_err();
        assert_eq!(error.kind, StructureErrorKind::EmptyHold);
        assert_eq!(error.span, span(42));
    }
    assert_eq!(
        instrument_without_else(vec![instr_debug]).unwrap_err().span,
        span(42)
    );
}

#[test]
fn test_lowering_preserves_payloads_spans_and_nested_fallbacks() {
    let mut exp = variable("annotated");
    exp.span = span(17);
    exp.note = std::rc::Rc::new(TypKind::Text);
    let block_ol = vec![conditional("nested", vec![ret("result")])];
    let block_sl = instrument(block_ol.clone(), None).unwrap().block;
    let not_exp = Mixfix::Arg(exp.clone());
    let guard = Guard::Sub(
        crate::phrase!(node: TypKind::Text, span: span(18)),
        Box::new(crate::lang::il::ast::Subcheck::Tuple(vec![
            crate::lang::il::ast::Subcheck::Skip,
        ])),
    );
    let instrs_ol = vec![
        ast_ol::InstrKind::Group(ast_ol::GroupInstr {
            id: id("G"),
            rel_signature: signature(),
            exps: vec![exp.clone()],
            block: block_ol.clone(),
        }),
        ast_ol::InstrKind::Let(ast_ol::LetInstr {
            exp_l: exp.clone(),
            exp_r: exp.clone(),
            iter_instrs: vec![],
            block: block_ol.clone(),
        }),
        ast_ol::InstrKind::Rule(ast_ol::RuleInstr {
            id: id("R"),
            not_exp: not_exp.clone(),
            input_hint: InputHint::new(vec![0]),
            iter_instrs: vec![],
            block: block_ol.clone(),
        }),
        ast_ol::InstrKind::Case(ast_ol::CaseInstr {
            exp: exp.clone(),
            cases: vec![ast_ol::Case {
                guard: guard.clone(),
                block: block_ol,
            }],
            total: false,
        }),
        ast_ol::InstrKind::Result(ast_ol::ResultInstr {
            rel_signature: signature(),
            exps: vec![exp.clone()],
        }),
        ast_ol::InstrKind::Return(ast_ol::ReturnInstr { exp: exp.clone() }),
    ];
    let instrs_sl = vec![
        InstrKind::Group(GroupInstr {
            id: id("G"),
            rel_signature: signature(),
            exps: vec![exp.clone()],
            block: block_sl.clone(),
        }),
        InstrKind::Let(LetInstr {
            exp_l: exp.clone(),
            exp_r: exp.clone(),
            iter_instrs: vec![],
            block: block_sl.clone(),
        }),
        InstrKind::Rule(RuleInstr {
            id: id("R"),
            not_exp,
            input_hint: InputHint::new(vec![0]),
            iter_instrs: vec![],
            block: block_sl.clone(),
        }),
        InstrKind::Case(CaseInstr {
            exp: exp.clone(),
            cases: vec![Case {
                guard,
                block: block_sl,
            }],
            dangle: true,
        }),
        InstrKind::Result(ResultInstr {
            rel_signature: signature(),
            exps: vec![exp.clone()],
        }),
        InstrKind::Return(ReturnInstr { exp }),
    ];
    let block_ol = instrs_ol
        .into_iter()
        .map(|instr_kind_ol| crate::phrase!(node: instr_kind_ol, span: span(19)))
        .collect();
    let block_expect_sl: Block = instrs_sl
        .into_iter()
        .map(|instr_kind_sl| crate::phrase!(node: instr_kind_sl, span: span(19)))
        .collect();
    assert_eq!(instrument(block_ol, None).unwrap().block, block_expect_sl);
}
