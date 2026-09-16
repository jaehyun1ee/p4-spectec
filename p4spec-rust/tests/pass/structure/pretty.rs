#[path = "pretty/rename_tick.rs"]
mod rename_tick;
#[path = "pretty/revive_underscore.rs"]
mod revive_underscore;

use super::{id, instr, ret, span, variable};
use crate::lang::{
    il::ast::{ArgKind, Exp, ExpKind, Id, Iter, TypKind, Var},
    traits::free::Free,
};
use crate::pass::structure::{
    ol::ast::*,
    pretty::{pretty_func, pretty_rel},
};

fn var_id(exp: &Exp) -> &Id {
    let ExpKind::Var(id) = &exp.node else {
        panic!("expected variable")
    };
    id
}

fn return_exp(instr_ol: &Instr) -> &Exp {
    let InstrKind::Return(instr_return) = &instr_ol.node else {
        panic!("expected return")
    };
    &instr_return.exp
}

fn binding(text_l: &str, text_r: &str, block: Block) -> Instr {
    instr(InstrKind::Let(LetInstr {
        exp_l: variable(text_l),
        exp_r: variable(text_r),
        iter_instrs: vec![],
        block,
    }))
}

fn iterator(text_bound: &str, text_bind: &str) -> InstrIter {
    let var = |text| Var {
        id: id(text),
        typ: crate::phrase! { node: TypKind::Bool, span: span(2) },
        iters: vec![],
    };
    InstrIter {
        iter: Iter::List,
        vars_bound: vec![var(text_bound)],
        vars_bind: vec![var(text_bind)],
    }
}

#[test]
fn test_prettification_reaches_fixed_point() {
    let (exps_match, block, block_else) = pretty_rel(
        vec![variable("_x'''")],
        vec![ret("_x'''")],
        Some(vec![ret("_x'''")]),
    )
    .unwrap();
    let body_again = pretty_rel(exps_match.clone(), block.clone(), block_else.clone()).unwrap();
    assert_eq!(
        (exps_match.clone(), block.clone(), block_else.clone()),
        body_again
    );
    assert_eq!(exps_match[0], variable("x"));
}

#[test]
fn test_function_inputs_main_and_else_share_names_and_preserve_def_arguments() {
    let arg_exp = crate::phrase! {node: ArgKind::Exp(Box::new(variable("_x'''"))), span: span(4)};
    let arg_def = crate::phrase! {node: ArgKind::Def(id("_func'''")), span: span(5)};
    let instr_debug = instr(InstrKind::Debug(DebugInstr {
        exp: variable("_x'''"),
        instr: Box::new(ret("_x'''")),
    }));
    let (args_input, block, block_else) = pretty_func(
        vec![arg_exp, arg_def.clone()],
        vec![instr_debug, ret("x")],
        Some(vec![ret("_x'''")]),
    )
    .unwrap();
    let ids = args_input[0].free();
    let id_input = ids.iter().next().unwrap();
    assert_eq!(id_input.node, "x'");
    assert_eq!(args_input[0].span, span(4));
    assert_eq!(args_input[1], arg_def);
    assert!(block[0].free().contains(id_input));
    assert!(block_else.as_ref().unwrap()[0].free().contains(id_input));
    let body_again = pretty_func(args_input.clone(), block.clone(), block_else.clone()).unwrap();
    assert_eq!(body_again, (args_input, block, block_else));
}

#[test]
fn test_fixed_point_retains_distinct_identifier_use_spans() {
    let mut exp_input = variable("x");
    let ExpKind::Var(id_input) = &mut exp_input.node else {
        unreachable!()
    };
    id_input.span = span(31);
    let mut exp_body = variable("x");
    let ExpKind::Var(id_body) = &mut exp_body.node else {
        unreachable!()
    };
    id_body.span = span(37);
    let block_expect = vec![instr(InstrKind::Return(ReturnInstr { exp: exp_body }))];
    let (exps_match, block, block_else) =
        pretty_rel(vec![exp_input.clone()], block_expect.clone(), Some(vec![])).unwrap();
    assert_eq!(exps_match, vec![exp_input]);
    assert_eq!(block, block_expect);
    assert_eq!(block_else, Some(vec![]));
}
