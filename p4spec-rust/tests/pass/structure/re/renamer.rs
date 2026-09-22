use super::super::{id, id_exp, instr, ret, span};
use crate::lang::{
    common::notation::mixfix::Mixfix,
    hints::input::InputHint,
    il::ast::{Exp, ExpKind, Id, Iter, TypKind, Var},
};
use crate::pass::structure::{ol::ast::*, re::renamer::Renamer};

fn id_of_exp(exp: &Exp) -> &Id {
    let ExpKind::Id(id) = &exp.node else { panic!("expected variable") };
    id
}

fn binding(text: &str, block: Block) -> Instr {
    instr(InstrKind::Let(LetInstr {
        exp_l: id_exp(text),
        exp_r: id_exp("x"),
        iter_instrs: vec![],
        block,
    }))
}

fn return_exp(instr_body: &Instr) -> &Exp {
    let InstrKind::Return(instr_body) = &instr_body.node else { panic!("expected return") };
    &instr_body.exp
}

#[test]
fn test_capture_avoidance_and_shadowing_preserve_spans() {
    let mut id_target = id("y");
    id_target.span = span(9);
    let renamer = Renamer::singleton(id("x"), id_target.clone());
    let block =
        vec![binding("y", vec![ret("x"), ret("y"), ret("z"), binding("x", vec![ret("x")])])];
    let block = renamer.rename_block(&mut false, block).unwrap();
    let InstrKind::Let(instr_body) = &block[0].node else { panic!("expected let") };
    let id_fresh = id_of_exp(&instr_body.exp_l);
    assert_ne!(id_fresh.node, "y");
    assert_eq!(id_fresh.span, span(9));
    assert_eq!(id_of_exp(&instr_body.exp_r), &id_target);
    assert_eq!(id_of_exp(return_exp(&instr_body.block[0])), &id_target);
    assert_eq!(id_of_exp(return_exp(&instr_body.block[1])), id_fresh);
    assert_eq!(id_of_exp(return_exp(&instr_body.block[2])).node, "z");
    assert_eq!(return_exp(&instr_body.block[0]).span, span(1));
    let InstrKind::Let(instr_inner) = &instr_body.block[3].node else { panic!("expected let") };
    assert_eq!(id_of_exp(return_exp(&instr_inner.block[0])).node, "x");
}

#[test]
fn test_freshness_avoids_domain_codomain_and_block_names() {
    let mut renamer = Renamer::singleton(id("x"), id("y"));
    renamer.add(id("y'"), id("z"));
    renamer.add(id("q"), id("y''"));
    let block = renamer
        .rename_block(&mut false, vec![binding("y", vec![ret("y'''"), ret("x"), ret("y")])])
        .unwrap();
    let InstrKind::Let(instr_body) = &block[0].node else { panic!("expected let") };
    let id_fresh = id_of_exp(&instr_body.exp_l);
    for text in ["y", "y'", "y''", "y'''"] {
        assert_ne!(id_fresh.node, text);
    }
    assert_eq!(id_of_exp(return_exp(&instr_body.block[2])), id_fresh);
}

fn iterator() -> InstrIter {
    let var = Var {
        id: id("y"),
        typ: crate::phrase! {node: TypKind::Bool, span: span(2)},
        iters: vec![],
    };
    InstrIter { iter: Iter::List, vars_bound: vec![var.clone()], vars_bind: vec![var] }
}

#[test]
fn test_iterator_bound_and_binding_are_distinct() {
    let renamer = Renamer::singleton(id("y"), id("z"));
    let iter_bound = renamer.rename_iterinstr_bound(&mut false, iterator());
    let iter_bind = renamer.rename_iterinstr_bind(&mut false, iterator());
    assert_eq!(iter_bound.vars_bound[0].id.node, "z");
    assert_eq!(iter_bound.vars_bind[0].id.node, "y");
    assert_eq!(iter_bind.vars_bound[0].id.node, "y");
    assert_eq!(iter_bind.vars_bind[0].id.node, "z");
}

#[test]
fn test_change_tracking_follows_filtered_and_capture_avoiding_renamers() {
    use crate::lang::traits::free::FreeIds;
    let mut changed = false;
    let renamer = Renamer::singleton(id("x"), id("y"));
    let renamer = renamer.filter(|_, _| true);
    assert!(!changed);
    renamer.rename_exp(&mut changed, id_exp("absent"));
    assert!(!changed);
    renamer.rename_exp(&mut changed, id_exp("x"));
    assert!(changed);

    changed = false;
    let renamer_fresh = renamer.freshen_binders(&id_exp("y").free_ids(), &vec![]);
    assert!(!changed);
    renamer_fresh.rename_exp(&mut changed, id_exp("y"));
    assert!(changed);

    changed = false;
    let mut id_target = id("y");
    id_target.span = span(77);
    let renamer = Renamer::singleton(id("y"), id_target.clone());
    let exp = renamer.rename_exp(&mut changed, id_exp("y"));
    assert_eq!(id_of_exp(&exp), &id_target);
    assert!(!changed);
}

#[test]
fn test_rule_outputs_freshen_under_hold_and_case() {
    let instr_rule = instr(InstrKind::Rule(RuleInstr {
        id: id("rel"),
        not_exp: Mixfix::Seq(vec![Mixfix::Arg(id_exp("x")), Mixfix::Arg(id_exp("y"))]),
        input_hint: InputHint::new(vec![crate::phrase!(node: 0, span: Default::default())]),
        iter_instrs: vec![iterator()],
        block: vec![ret("x"), ret("y")],
    }));
    let instr_hold = instr(InstrKind::Hold(HoldInstr {
        id: id("rel"),
        not_exp: Mixfix::Arg(id_exp("x")),
        iter_exps: vec![],
        block_hold: vec![instr_rule],
        block_not_hold: vec![ret("y")],
    }));
    let instr_case = instr(InstrKind::Case(CaseInstr {
        exp: id_exp("x"),
        cases: vec![Case { guard: Guard::Mem(id_exp("x")), block: vec![instr_hold] }],
        total: true,
    }));
    let block = Renamer::singleton(id("x"), id("y"))
        .rename_block(&mut false, vec![instr_case])
        .unwrap();
    let InstrKind::Case(instr_case) = &block[0].node else { panic!("expected case") };
    let Guard::Mem(exp) = &instr_case.cases[0].guard else { panic!("expected membership") };
    assert_eq!(id_of_exp(exp).node, "y");
    let InstrKind::Hold(instr_hold) = &instr_case.cases[0].block[0].node else {
        panic!("expected hold")
    };
    assert_eq!(id_of_exp(return_exp(&instr_hold.block_not_hold[0])).node, "y");
    let InstrKind::Rule(instr_rule) = &instr_hold.block_hold[0].node else {
        panic!("expected rule")
    };
    let exps = instr_rule.not_exp.args();
    let id_fresh = id_of_exp(exps[1]);
    assert_eq!(id_of_exp(exps[0]).node, "y");
    assert_ne!(id_fresh.node, "y");
    assert_eq!(&instr_rule.iter_instrs[0].vars_bound[0].id, id_fresh);
    assert_eq!(instr_rule.iter_instrs[0].vars_bind[0].id.node, "y");
    assert_eq!(id_of_exp(return_exp(&instr_rule.block[0])).node, "y");
    assert_eq!(id_of_exp(return_exp(&instr_rule.block[1])), id_fresh);
}

#[test]
fn test_let_iterator_bound_tracks_fresh_binder() {
    let instr_let = instr(InstrKind::Let(LetInstr {
        exp_l: id_exp("y"),
        exp_r: id_exp("x"),
        iter_instrs: vec![iterator()],
        block: vec![binding("y", vec![ret("x"), ret("y")]), ret("y")],
    }));
    let block = Renamer::singleton(id("x"), id("y"))
        .rename_block(&mut false, vec![instr_let])
        .unwrap();
    let InstrKind::Let(instr_outer) = &block[0].node else { panic!("expected let") };
    let id_outer = id_of_exp(&instr_outer.exp_l);
    assert_eq!(&instr_outer.iter_instrs[0].vars_bound[0].id, id_outer);
    assert_eq!(instr_outer.iter_instrs[0].vars_bind[0].id.node, "y");
    let InstrKind::Let(instr_inner) = &instr_outer.block[0].node else { panic!("expected let") };
    assert_eq!(id_of_exp(return_exp(&instr_inner.block[1])), id_of_exp(&instr_inner.exp_l));
    assert_eq!(id_of_exp(return_exp(&instr_outer.block[1])), id_outer);
}

#[test]
fn test_expression_paths_arguments_and_iterator_annotations() {
    use crate::lang::il::ast::{ArgKind, PathKind, Subcheck};
    let typ = crate::phrase! {node: TypKind::Bool, span: span(4)};
    let path_root = crate::note_phrase! {node: PathKind::Root, note: TypKind::Bool, span: span(5)};
    let path = crate::note_phrase! {
        node: PathKind::Slice(Box::new(path_root), Box::new(id_exp("x")), Box::new(id_exp("z"))),
        note: TypKind::Bool, span: span(6)
    };
    let exp_update = crate::note_phrase! {
        node: ExpKind::Upd(Box::new(id_exp("x")), Box::new(path), Box::new(id_exp("z"))),
        note: TypKind::Bool, span: span(3)
    };
    let iter_instr = iterator();
    let exp_iter = crate::note_phrase! {
        node: ExpKind::Iter(Box::new(exp_update), ExpIter { iter: iter_instr.iter, vars: iter_instr.vars_bound }),
        note: TypKind::Bool, span: span(2)
    };
    let exp = crate::note_phrase! {
        node: ExpKind::Call(id("x"), vec![typ.clone()], vec![
            crate::phrase! {node: ArgKind::Exp(Box::new(exp_iter)), span: span(8)},
            crate::phrase! {node: ArgKind::Def(id("x")), span: span(9)}
        ]),
        note: TypKind::Bool, span: span(10)
    };
    let mut renamer = Renamer::singleton(id("x"), id("q"));
    renamer.add(id("y"), id("r"));
    let exp = renamer.rename_exp(&mut false, exp);
    assert_eq!(exp.span, span(10));
    let ExpKind::Call(id_func, targs, args) = exp.node else { panic!("expected call") };
    assert_eq!(id_func.node, "x");
    assert_eq!(targs, vec![typ.clone()]);
    assert_eq!(args[0].span, span(8));
    assert_eq!(args[1].span, span(9));
    let ArgKind::Def(id_def) = &args[1].node else { panic!("expected def") };
    assert_eq!(id_def.node, "x");
    let ArgKind::Exp(exp) = &args[0].node else { panic!("expected exp") };
    assert_eq!(exp.span, span(2));
    let ExpKind::Iter(exp, ExpIter { vars, .. }) = &exp.node else { panic!("expected iter") };
    assert_eq!(vars[0].id.node, "r");
    assert_eq!(vars[0].typ.span, span(2));
    assert_eq!(exp.span, span(3));
    let ExpKind::Upd(exp_base, path, exp_field) = &exp.node else { panic!("expected update") };
    assert_eq!(id_of_exp(exp_base).node, "q");
    assert_eq!(id_of_exp(exp_field).node, "z");
    assert_eq!(path.span, span(6));
    let PathKind::Slice(path, exp_idx, exp_len) = &path.node else { panic!("expected slice") };
    assert_eq!(path.span, span(5));
    assert_eq!(id_of_exp(exp_idx).node, "q");
    assert_eq!(id_of_exp(exp_len).node, "z");
    let guard = Guard::Sub(typ, Box::new(Subcheck::Iter(Iter::List, Box::new(Subcheck::Skip))));
    assert_eq!(renamer.rename_guard(&mut false, guard.clone()), guard);
}

#[test]
fn test_nested_rule_shadows_rename_inside_let_iterator() {
    let mut iter_instr = iterator();
    iter_instr.vars_bound[0].id = id("x");
    iter_instr.vars_bind[0].id = id("x");
    let instr_rule = instr(InstrKind::Rule(RuleInstr {
        id: id("rel"),
        not_exp: Mixfix::Seq(vec![Mixfix::Arg(id_exp("x")), Mixfix::Arg(id_exp("x"))]),
        input_hint: InputHint::new(vec![crate::phrase!(node: 0, span: Default::default())]),
        iter_instrs: vec![iter_instr],
        block: vec![ret("x"), ret("y")],
    }));
    let instr_let = instr(InstrKind::Let(LetInstr {
        exp_l: id_exp("y"),
        exp_r: id_exp("x"),
        iter_instrs: vec![iterator()],
        block: vec![instr_rule, ret("x")],
    }));
    let block = Renamer::singleton(id("x"), id("y"))
        .rename_block(&mut false, vec![instr_let])
        .unwrap();
    let InstrKind::Let(instr_let) = &block[0].node else { panic!("expected let") };
    let id_fresh = id_of_exp(&instr_let.exp_l);
    let InstrKind::Rule(instr_rule) = &instr_let.block[0].node else { panic!("expected rule") };
    let exps = instr_rule.not_exp.args();
    assert_eq!(id_of_exp(exps[0]).node, "y");
    assert_eq!(id_of_exp(exps[1]).node, "x");
    assert_eq!(instr_rule.iter_instrs[0].vars_bound[0].id.node, "x");
    assert_eq!(instr_rule.iter_instrs[0].vars_bind[0].id.node, "x");
    assert_eq!(id_of_exp(return_exp(&instr_rule.block[0])).node, "x");
    assert_eq!(id_of_exp(return_exp(&instr_rule.block[1])), id_fresh);
    assert_eq!(id_of_exp(return_exp(&instr_let.block[1])).node, "y");
}

#[test]
fn test_empty_renaming_moves_notation_payloads() {
    let exp = crate::note_phrase! {
        node: ExpKind::Case(Box::new(Mixfix::Arg(id_exp("payload")))),
        note: TypKind::Bool, span: span(3)
    };
    let ExpKind::Case(not_exp) = &exp.node else { unreachable!() };
    let ptr = id_of_exp(not_exp.args()[0]).node.as_ptr();
    let exp_expect = exp.clone();
    let exp = Renamer::empty().rename_exp(&mut false, exp);
    assert_eq!(exp, exp_expect);
    let ExpKind::Case(not_exp) = &exp.node else { unreachable!() };
    assert_eq!(id_of_exp(not_exp.args()[0]).node.as_ptr(), ptr);

    let instr_rule = instr(InstrKind::Rule(RuleInstr {
        id: id("rel"),
        not_exp: Mixfix::Arg(id_exp("input")),
        input_hint: InputHint::new(vec![crate::phrase!(node: 0, span: Default::default())]),
        iter_instrs: vec![],
        block: vec![binding("bound", vec![ret("bound")])],
    }));
    let InstrKind::Rule(instr_body) = &instr_rule.node else { unreachable!() };
    let ptr = id_of_exp(instr_body.not_exp.args()[0]).node.as_ptr();
    let instr_expect = instr_rule.clone();
    let instr_rule = Renamer::empty()
        .rename_instr(&mut false, instr_rule)
        .unwrap();
    assert_eq!(instr_rule, instr_expect);
    let InstrKind::Rule(instr_body) = &instr_rule.node else { unreachable!() };
    assert_eq!(id_of_exp(instr_body.not_exp.args()[0]).node.as_ptr(), ptr);
}
