use super::super::{id, id_exp, instr, ret, span};
use crate::lang::{
    common::notation::mixfix::Mixfix,
    hints::input::InputHint,
    il::ast::{ExpKind, Iter, TypKind, Var},
};
use crate::pass::structure::ol::ast::*;
fn binding(text: &str, block: Block) -> Instr {
    instr(InstrKind::Let(LetInstr {
        exp_l: id_exp(text),
        exp_r: id_exp("input"),
        iter_instrs: vec![],
        block,
    }))
}

fn rule(text: &str, block: Block) -> Instr {
    instr(InstrKind::Rule(RuleInstr {
        id: id("relation"),
        not_exp: Mixfix::Seq(vec![Mixfix::Arg(id_exp("input")), Mixfix::Arg(id_exp(text))]),
        input_hint: InputHint::new(vec![crate::phrase!(node: 0, span: Default::default())]),
        iter_instrs: vec![],
        block,
    }))
}

fn hold(block_hold: Block, block_not_hold: Block) -> Instr {
    instr(InstrKind::Hold(HoldInstr {
        id: id("relation"),
        not_exp: Mixfix::Arg(id_exp("input")),
        iter_exps: vec![],
        block_hold,
        block_not_hold,
    }))
}

fn var(text: &str) -> Var {
    Var { id: id(text), typ: crate::phrase! {node:TypKind::Bool,span:span(2)}, iters: vec![] }
}

#[path = "loop/merge_binding.rs"]
mod merge_binding;
#[path = "loop/merge_hold.rs"]
mod merge_hold;

#[path = "loop/casify.rs"]
mod casify;
#[path = "loop/merge_if.rs"]
mod merge_if;

#[test]
fn test_wide_flat_blocks_preserve_order_without_recursive_sibling_traversal() {
    use crate::{pass::structure::opt::r#loop, runtime::envs::algo::TDEnv};

    let block: Block = (0..4096).map(|num_idx| ret(&num_idx.to_string())).collect();
    let tdenv = TDEnv::new();
    assert_eq!(r#loop::merge_if::apply(&tdenv, &mut false, block.clone()).unwrap(), block);
    assert_eq!(r#loop::casify::apply(&tdenv, &mut false, block.clone()).unwrap(), block);
    assert_eq!(r#loop::merge_binding::apply(&mut false, block.clone()).unwrap(), block);
    assert_eq!(r#loop::merge_hold::apply(&mut false, block.clone()), block);
}
