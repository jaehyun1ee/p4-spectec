use super::{instr, ret, span, variable};
use crate::lang::il::ast::Iter;
use crate::pass::structure::{merge::merge_blocks, ol::ast::*};

fn if_instr(text_cond: &str, iter: Iter, block: Block, num_line: i64) -> Instr {
    let mut instr_if = instr(InstrKind::If(IfInstr {
        exp: variable(text_cond),
        iter_exps: vec![(iter, vec![])],
        block,
    }));
    instr_if.span = span(num_line);
    instr_if
}

fn return_name(instr_body: &Instr) -> &str {
    let InstrKind::Return(instr_return) = &instr_body.node else {
        panic!("expected return")
    };
    let crate::lang::il::ast::ExpKind::Var(id_return) = &instr_return.exp.node else {
        panic!("expected variable")
    };
    &id_return.node
}

#[test]
fn test_equal_if_prefixes_merge_three_paths_and_preserve_first_span_and_tail_order() {
    let block_a = vec![
        if_instr(
            "outer",
            Iter::List,
            vec![
                if_instr("inner", Iter::Opt, vec![ret("a")], 111),
                ret("inner_tail_a"),
            ],
            11,
        ),
        ret("tail_a"),
    ];
    let block_b = vec![
        if_instr(
            "outer",
            Iter::List,
            vec![
                if_instr("inner", Iter::Opt, vec![ret("b")], 222),
                ret("inner_tail_b"),
            ],
            22,
        ),
        ret("tail_b"),
    ];
    let block_c = vec![
        if_instr(
            "outer",
            Iter::List,
            vec![
                if_instr("inner", Iter::Opt, vec![ret("c")], 333),
                ret("inner_tail_c"),
            ],
            33,
        ),
        ret("tail_c"),
    ];

    let block = merge_blocks(vec![block_a, block_b, block_c]);

    assert_eq!(block.len(), 4);
    assert_eq!(block[0].span, span(11));
    let InstrKind::If(instr_if) = &block[0].node else {
        panic!("expected merged if")
    };
    assert_eq!(instr_if.block.len(), 4);
    assert_eq!(instr_if.block[0].span, span(111));
    let InstrKind::If(instr_if_inner) = &instr_if.block[0].node else {
        panic!("expected merged inner if")
    };
    assert_eq!(
        instr_if_inner
            .block
            .iter()
            .map(return_name)
            .collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
    assert_eq!(
        instr_if.block[1..]
            .iter()
            .map(return_name)
            .collect::<Vec<_>>(),
        ["inner_tail_a", "inner_tail_b", "inner_tail_c"]
    );
    assert_eq!(
        block[1..].iter().map(return_name).collect::<Vec<_>>(),
        ["tail_a", "tail_b", "tail_c"]
    );
}

#[test]
fn test_distinct_condition_or_iterator_concatenates_paths_in_order() {
    let block_a = vec![if_instr("a", Iter::List, vec![ret("body_a")], 1)];
    let block_b = vec![if_instr("b", Iter::List, vec![ret("body_b")], 2)];
    let block_c = vec![if_instr("b", Iter::Opt, vec![ret("body_c")], 3)];

    let block = merge_blocks(vec![block_a, block_b, block_c]);

    assert_eq!(block.len(), 3);
    assert_eq!(block[0].span, span(1));
    assert_eq!(block[1].span, span(2));
    assert_eq!(block[2].span, span(3));
}
