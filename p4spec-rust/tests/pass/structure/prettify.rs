use super::{ret, variable};
use crate::pass::structure::prettify::pretty_rel;
#[test]
fn test_prettification_reaches_fixed_point() {
    let body = pretty_rel(
        vec![variable("_x'''")],
        vec![ret("_x'''")],
        Some(vec![ret("_x'''")]),
    )
    .unwrap();
    let body_again = pretty_rel(
        body.exps_match.clone(),
        body.block.clone(),
        body.block_else.clone(),
    )
    .unwrap();
    assert_eq!(body, body_again);
    assert_eq!(body.exps_match[0], variable("x"));
}

use super::{id, instr, span};
use crate::lang::{
    il::ast::{ArgKind, ExpKind},
    traits::free::Free,
};
use crate::pass::structure::{ol::ast::*, prettify::pretty_func};
#[test]
fn test_function_inputs_main_and_else_share_names_and_preserve_def_arguments() {
    let arg_exp = crate::phrase! {node: ArgKind::Exp(Box::new(variable("_x'''"))), span: span(4)};
    let arg_def = crate::phrase! {node: ArgKind::Def(id("_func'''")), span: span(5)};
    let instr_debug = instr(InstrKind::Debug(DebugInstr {
        exp: variable("_x'''"),
        instr: Box::new(ret("_x'''")),
    }));
    let body = pretty_func(
        vec![arg_exp, arg_def.clone()],
        vec![instr_debug, ret("x")],
        Some(vec![ret("_x'''")]),
    )
    .unwrap();
    let ids = body.args_input[0].free();
    let id_input = ids.iter().next().unwrap();
    assert_eq!(id_input.node, "x'");
    assert_eq!(body.args_input[0].span, span(4));
    assert_eq!(body.args_input[1], arg_def);
    assert!(body.block[0].free().contains(id_input));
    assert!(
        body.block_else.as_ref().unwrap()[0]
            .free()
            .contains(id_input)
    );
    let body_again = pretty_func(
        body.args_input.clone(),
        body.block.clone(),
        body.block_else.clone(),
    )
    .unwrap();
    assert_eq!(body_again, body);
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
    let block = vec![instr(InstrKind::Return(ReturnInstr { exp: exp_body }))];
    let body = pretty_rel(vec![exp_input.clone()], block.clone(), Some(vec![])).unwrap();
    assert_eq!(body.exps_match, vec![exp_input]);
    assert_eq!(body.block, block);
    assert_eq!(body.block_else, Some(vec![]));
}
