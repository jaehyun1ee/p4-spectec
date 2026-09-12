use super::super::{id, instr, ret, span, variable};
use crate::lang::{
    common::notation::mixfix::Mixfix,
    hints::input::InputHint,
    il::ast::{Exp, ExpKind, Id, Iter, TypKind, Var},
};
use crate::pass::structure::{ol::ast::*, re::replacer::Replacer};

fn var_id(exp: &Exp) -> &Id {
    let ExpKind::Var(id) = &exp.node else {
        panic!("expected variable")
    };
    id
}
fn binding(text: &str, block: Block) -> Instr {
    instr(InstrKind::Let(LetInstr {
        exp_l: variable(text),
        exp_r: variable("x"),
        iter_instrs: vec![],
        block,
    }))
}
fn return_exp(instr_body: &Instr) -> &Exp {
    let InstrKind::Return(instr_body) = &instr_body.node else {
        panic!("expected return")
    };
    &instr_body.exp
}
#[test]
fn test_capture_avoidance_and_shadowed_rhs() {
    let mut exp_target = variable("y");
    exp_target.span = span(9);
    exp_target.note = TypKind::Text.into();
    let mut replacer = Replacer::singleton(id("x"), exp_target.clone());
    replacer.add(id("y"), variable("z"));
    let block = replacer
        .replace_block(vec![
            binding("y", vec![ret("x"), ret("y")]),
            binding("x", vec![ret("x")]),
            ret("x"),
        ])
        .unwrap();
    let InstrKind::Let(instr_body) = &block[0].node else {
        panic!("expected let")
    };
    let id_fresh = var_id(&instr_body.exp_l);
    assert_ne!(id_fresh.node, "y");
    assert_eq!(&instr_body.exp_r, &exp_target);
    assert_eq!(return_exp(&instr_body.block[0]), &exp_target);
    assert_eq!(var_id(return_exp(&instr_body.block[1])), id_fresh);
    let InstrKind::Let(instr_shadow) = &block[1].node else {
        panic!("expected let")
    };
    assert_eq!(var_id(&instr_shadow.exp_r).node, "x");
    assert_eq!(var_id(return_exp(&instr_shadow.block[0])).node, "x");
    assert_eq!(return_exp(&block[2]), &exp_target);
}
fn iterator() -> InstrIter {
    let var = Var {
        id: id("x"),
        typ: crate::phrase! {node: TypKind::Bool, span: span(2)},
        iters: vec![],
    };
    InstrIter {
        iter: Iter::List,
        vars_bound: vec![var.clone()],
        vars_bind: vec![var],
    }
}
#[test]
fn test_iterator_filters_third_component_and_preserves_scope() {
    let replacer = Replacer::singleton(id("x"), variable("z"));
    let iter_instr = replacer.replace_iterinstr_bound(iterator());
    assert_eq!(iter_instr.vars_bound[0].id.node, "x");
    assert!(iter_instr.vars_bind.is_empty());
    let iter_instr = iterator();
    assert!(
        replacer
            .replace_iterexp((iter_instr.iter, iter_instr.vars_bound))
            .1
            .is_empty()
    );
    let mut instr_let = binding("x", vec![ret("x")]);
    let InstrKind::Let(instr_body) = &mut instr_let.node else {
        panic!("expected let")
    };
    instr_body.iter_instrs = vec![iterator()];
    let block = replacer.replace_block(vec![instr_let, ret("x")]).unwrap();
    let InstrKind::Let(instr_body) = &block[0].node else {
        panic!("expected let")
    };
    assert_eq!(instr_body.iter_instrs[0].vars_bind.len(), 1);
    assert_eq!(var_id(return_exp(&block[1])).node, "z");
}
#[test]
fn test_rule_inputs_precede_output_shadowing_and_freshening() {
    let instr_rule = instr(InstrKind::Rule(RuleInstr {
        id: id("rel"),
        not_exp: Mixfix::Seq(vec![Mixfix::Arg(variable("x")), Mixfix::Arg(variable("y"))]),
        input_hint: InputHint::new(vec![0]),
        iter_instrs: vec![iterator()],
        block: vec![ret("x"), ret("y")],
    }));
    let mut replacer = Replacer::singleton(id("x"), variable("y"));
    replacer.add(id("y"), variable("z"));
    let instr_rule = replacer.replace_instr(instr_rule).unwrap();
    let InstrKind::Rule(instr_rule) = instr_rule.node else {
        panic!("expected rule")
    };
    let exps = instr_rule.not_exp.args();
    assert_eq!(var_id(exps[0]).node, "y");
    let id_fresh = var_id(exps[1]);
    assert_ne!(id_fresh.node, "y");
    assert_eq!(var_id(return_exp(&instr_rule.block[1])), id_fresh);
    assert_eq!(var_id(return_exp(&instr_rule.block[0])).node, "y");
    assert!(instr_rule.iter_instrs[0].vars_bind.is_empty());
    let instr_rule = instr(InstrKind::Rule(RuleInstr {
        id: id("rel"),
        not_exp: Mixfix::Seq(vec![Mixfix::Arg(variable("x")), Mixfix::Arg(variable("x"))]),
        input_hint: InputHint::new(vec![0]),
        iter_instrs: vec![],
        block: vec![ret("x")],
    }));
    let instr_rule = replacer.replace_instr(instr_rule).unwrap();
    let InstrKind::Rule(instr_rule) = instr_rule.node else {
        panic!("expected rule")
    };
    assert_eq!(var_id(instr_rule.not_exp.args()[0]).node, "y");
    assert_eq!(var_id(instr_rule.not_exp.args()[1]).node, "x");
    assert_eq!(var_id(return_exp(&instr_rule.block[0])).node, "x");
}

#[test]
fn test_invalid_rule_hint_reports_instruction_span() {
    use crate::lang::hints::input::InputError;
    use crate::pass::structure::StructureErrorKind;
    let mut instr_rule = instr(InstrKind::Rule(RuleInstr {
        id: id("rel"),
        not_exp: Mixfix::Arg(variable("x")),
        input_hint: InputHint::new(vec![2]),
        iter_instrs: vec![],
        block: vec![],
    }));
    instr_rule.span = span(7);
    let error = Replacer::empty().replace_instr(instr_rule).unwrap_err();
    assert_eq!(error.span, span(7));
    assert_eq!(
        error.kind,
        StructureErrorKind::Input(InputError::IndexOutOfBounds { index: 2, arity: 1 })
    );
}

#[test]
fn test_expression_paths_arguments_and_iterator_annotations() {
    use crate::lang::il::ast::{ArgKind, PathKind, Subcheck};
    let typ = crate::phrase! {node: TypKind::Bool, span: span(4)};
    let path_root = crate::note_phrase! {node: PathKind::Root, note: TypKind::Bool, span: span(5)};
    let path = crate::note_phrase! {
        node: PathKind::Slice(Box::new(path_root), Box::new(variable("x")), Box::new(variable("z"))),
        note: TypKind::Bool, span: span(6)
    };
    let exp_update = crate::note_phrase! {
        node: ExpKind::Upd(Box::new(variable("x")), Box::new(path), Box::new(variable("z"))),
        note: TypKind::Bool, span: span(3)
    };
    let iter_instr = iterator();
    let exp_iter = crate::note_phrase! {
        node: ExpKind::Iter(Box::new(exp_update), (iter_instr.iter, iter_instr.vars_bound)),
        note: TypKind::Bool, span: span(2)
    };
    let exp = crate::note_phrase! {
        node: ExpKind::Call(id("x"), vec![typ.clone()], vec![
            crate::phrase! {node: ArgKind::Exp(Box::new(exp_iter)), span: span(8)},
            crate::phrase! {node: ArgKind::Def(id("x")), span: span(9)}
        ]),
        note: TypKind::Bool, span: span(10)
    };
    let mut replacer = Replacer::singleton(id("x"), variable("q"));
    replacer.add(id("y"), variable("r"));
    let exp = replacer.replace_exp(exp);
    assert_eq!(exp.span, span(10));
    let ExpKind::Call(id_func, targs, args) = exp.node else {
        panic!("expected call")
    };
    assert_eq!(id_func.node, "x");
    assert_eq!(targs, vec![typ.clone()]);
    assert_eq!(args[0].span, span(8));
    assert_eq!(args[1].span, span(9));
    let ArgKind::Def(id_def) = &args[1].node else {
        panic!("expected def")
    };
    assert_eq!(id_def.node, "x");
    let ArgKind::Exp(exp) = &args[0].node else {
        panic!("expected exp")
    };
    assert_eq!(exp.span, span(2));
    let ExpKind::Iter(exp, (_, vars)) = &exp.node else {
        panic!("expected iter")
    };
    assert!(vars.is_empty());
    assert_eq!(exp.span, span(3));
    let ExpKind::Upd(exp_base, path, exp_field) = &exp.node else {
        panic!("expected update")
    };
    assert_eq!(var_id(exp_base).node, "q");
    assert_eq!(var_id(exp_field).node, "z");
    assert_eq!(path.span, span(6));
    let PathKind::Slice(path, exp_idx, exp_len) = &path.node else {
        panic!("expected slice")
    };
    assert_eq!(path.span, span(5));
    assert_eq!(var_id(exp_idx).node, "q");
    assert_eq!(var_id(exp_len).node, "z");
    let guard = Guard::Sub(
        typ,
        Box::new(Subcheck::Iter(Iter::List, Box::new(Subcheck::Skip))),
    );
    assert_eq!(replacer.replace_guard(guard.clone()), guard);
}

#[test]
fn test_rule_outputs_freshen_under_hold_and_case() {
    let mut iter_instr = iterator();
    iter_instr.vars_bound[0].id = id("y");
    iter_instr.vars_bind[0].id = id("y");
    let instr_rule = instr(InstrKind::Rule(RuleInstr {
        id: id("rel"),
        not_exp: Mixfix::Seq(vec![Mixfix::Arg(variable("x")), Mixfix::Arg(variable("y"))]),
        input_hint: InputHint::new(vec![0]),
        iter_instrs: vec![iter_instr],
        block: vec![ret("x"), ret("y")],
    }));
    let instr_hold = instr(InstrKind::Hold(HoldInstr {
        id: id("rel"),
        not_exp: Mixfix::Arg(variable("x")),
        iter_exps: vec![],
        block_hold: vec![instr_rule],
        block_not_hold: vec![ret("y")],
    }));
    let instr_case = instr(InstrKind::Case(CaseInstr {
        exp: variable("x"),
        cases: vec![Case {
            guard: Guard::Mem(variable("x")),
            block: vec![instr_hold],
        }],
        total: true,
    }));
    let block = Replacer::singleton(id("x"), variable("y"))
        .replace_block(vec![instr_case])
        .unwrap();
    let InstrKind::Case(instr_case) = &block[0].node else {
        panic!("expected case")
    };
    let Guard::Mem(exp) = &instr_case.cases[0].guard else {
        panic!("expected membership")
    };
    assert_eq!(var_id(exp).node, "y");
    let InstrKind::Hold(instr_hold) = &instr_case.cases[0].block[0].node else {
        panic!("expected hold")
    };
    assert_eq!(var_id(return_exp(&instr_hold.block_not_hold[0])).node, "y");
    let InstrKind::Rule(instr_rule) = &instr_hold.block_hold[0].node else {
        panic!("expected rule")
    };
    let exps = instr_rule.not_exp.args();
    let id_fresh = var_id(exps[1]);
    assert_eq!(var_id(exps[0]).node, "y");
    assert_ne!(id_fresh.node, "y");
    assert_eq!(&instr_rule.iter_instrs[0].vars_bound[0].id, id_fresh);
    assert_eq!(instr_rule.iter_instrs[0].vars_bind[0].id.node, "y");
    assert_eq!(var_id(return_exp(&instr_rule.block[0])).node, "y");
    assert_eq!(var_id(return_exp(&instr_rule.block[1])), id_fresh);
}

#[test]
fn test_compound_codomain_and_freshness_avoid_all_names() {
    let exp_target = crate::note_phrase! {
        node: ExpKind::Tuple(vec![variable("y"), variable("y''")]),
        note: TypKind::Bool, span: span(9)
    };
    let mut replacer = Replacer::singleton(id("x"), exp_target.clone());
    replacer.add(id("y'"), variable("q"));
    let block = replacer
        .replace_block(vec![binding("y", vec![ret("y'''"), ret("x"), ret("y")])])
        .unwrap();
    let InstrKind::Let(instr_body) = &block[0].node else {
        panic!("expected let")
    };
    let id_fresh = var_id(&instr_body.exp_l);
    for text in ["y", "y'", "y''", "y'''"] {
        assert_ne!(id_fresh.node, text);
    }
    assert_eq!(return_exp(&instr_body.block[1]), &exp_target);
    assert_eq!(var_id(return_exp(&instr_body.block[2])), id_fresh);
}
