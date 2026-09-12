use super::*;
use crate::pass::structure::opt::pre::remove_group::apply;
#[test]
fn test_nested_order_and_debug_barrier() {
    let instr_debug = instr(InstrKind::Debug(DebugInstr {
        exp: variable("debug"),
        instr: Box::new(group(vec![ret("hidden")])),
    }));
    let instr_if = instr(InstrKind::If(IfInstr {
        exp: variable("condition"),
        iter_exps: vec![(Iter::List, vec![])],
        block: vec![group(vec![ret("a"), group(vec![ret("b")])]), ret("c")],
    }));
    let block = apply(vec![
        group(vec![ret("head"), instr_if]),
        instr_debug.clone(),
        ret("tail"),
    ]);
    assert_eq!(block[0], ret("head"));
    let InstrKind::If(instr_if) = &block[1].node else {
        panic!("expected if")
    };
    assert_eq!(instr_if.block, vec![ret("a"), ret("b"), ret("c")]);
    assert_eq!(instr_if.iter_exps, vec![(Iter::List, vec![])]);
    assert_eq!(block[2], instr_debug);
    assert_eq!(block[3], ret("tail"));
}

#[test]
fn test_all_nested_blocks_preserve_annotations() {
    assert_eq!(
        apply(vec![group(containers(vec![group(vec![ret("leaf")])]))]),
        containers(vec![ret("leaf")])
    );
}
