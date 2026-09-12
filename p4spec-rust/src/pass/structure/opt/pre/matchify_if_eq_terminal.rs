use crate::lang::common::source::{NotePhrase, Span};
use crate::lang::{
    il::ast::{CmpOp, ExpKind, ListPattern, OpTyp, OptPattern, Pattern, UnOp},
    xl::bool::{CmpOp as BoolCmpOp, UnOp as BoolUnOp},
};
use crate::pass::structure::ol::ast::*;

fn matchify_exp(exp: Exp) -> Exp {
    let NotePhrase {
        node: exp_kind,
        note,
        span,
    } = exp;
    let exp_kind = match exp_kind {
        ExpKind::Cmp(op, op_typ, exp_l, exp_r) => {
            matchify_cmp_exp(op, op_typ, exp_l, exp_r, &note, &span)
        }
        _ => exp_kind,
    };
    NotePhrase {
        node: exp_kind,
        note,
        span,
    }
}
fn matchify_cmp_exp(
    op: CmpOp,
    op_typ: OpTyp,
    exp_l: Box<Exp>,
    exp_r: Box<Exp>,
    note: &std::rc::Rc<crate::lang::il::ast::TypKind>,
    span: &Span,
) -> ExpKind {
    let CmpOp::Bool(op_bool) = op else {
        return ExpKind::Cmp(op, op_typ, exp_l, exp_r);
    };
    // Option and list rules precede terminal cases, including across operands
    let pattern = option_pattern(&exp_r, op_bool)
        .map(|pattern| (false, pattern))
        .or_else(|| option_pattern(&exp_l, op_bool).map(|pattern| (true, pattern)))
        .or_else(|| list_pattern(&exp_r, op_bool).map(|pattern| (false, pattern)))
        .or_else(|| list_pattern(&exp_l, op_bool).map(|pattern| (true, pattern)));
    if let Some((reversed, pattern)) = pattern {
        return ExpKind::Match(if reversed { exp_r } else { exp_l }, pattern);
    }
    let pattern = terminal_pattern(&exp_r)
        .map(|pattern| (false, pattern))
        .or_else(|| terminal_pattern(&exp_l).map(|pattern| (true, pattern)));
    let Some((reversed, pattern)) = pattern else {
        return ExpKind::Cmp(op, op_typ, exp_l, exp_r);
    };
    let exp_kind = ExpKind::Match(if reversed { exp_r } else { exp_l }, pattern);
    match op_bool {
        BoolCmpOp::Eq => exp_kind,
        BoolCmpOp::Ne => {
            let exp = NotePhrase {
                node: exp_kind,
                note: note.clone(),
                span: span.clone(),
            };
            ExpKind::Un(UnOp::Bool(BoolUnOp::Not), OpTyp::Bool, Box::new(exp))
        }
    }
}
fn option_pattern(exp: &Exp, op: BoolCmpOp) -> Option<Pattern> {
    match &exp.node {
        ExpKind::Opt(None) => Some(Pattern::Opt(match op {
            BoolCmpOp::Eq => OptPattern::None,
            BoolCmpOp::Ne => OptPattern::Some,
        })),
        _ => None,
    }
}
fn list_pattern(exp: &Exp, op: BoolCmpOp) -> Option<Pattern> {
    match &exp.node {
        ExpKind::List(exps) if exps.is_empty() => Some(Pattern::List(match op {
            BoolCmpOp::Eq => ListPattern::Nil,
            BoolCmpOp::Ne => ListPattern::Cons,
        })),
        _ => None,
    }
}
fn terminal_pattern(exp: &Exp) -> Option<Pattern> {
    match &exp.node {
        ExpKind::Case(not_exp) if not_exp.arity() == 0 => {
            Some(Pattern::Case(Box::new(not_exp.to_mixop())))
        }
        _ => None,
    }
}

fn matchify_instr(instr_ol: Instr) -> Instr {
    let NotePhrase {
        node: instr_kind_ol,
        note: (),
        span,
    } = instr_ol;
    matchify_instr_kind(instr_kind_ol, span)
}
fn matchify_instr_kind(instr_kind_ol: InstrKind, span: Span) -> Instr {
    match instr_kind_ol {
        InstrKind::If(instr_ol) => matchify_if_instr(instr_ol, span),
        InstrKind::Hold(instr_ol) => matchify_hold_instr(instr_ol, span),
        InstrKind::Case(instr_ol) => matchify_case_instr(instr_ol, span),
        InstrKind::Group(instr_ol) => matchify_group_instr(instr_ol, span),
        InstrKind::Let(instr_ol) => matchify_let_instr(instr_ol, span),
        InstrKind::Rule(instr_ol) => matchify_rule_instr(instr_ol, span),
        InstrKind::Result(_) | InstrKind::Return(_) | InstrKind::Debug(_) => {
            crate::phrase! {node: instr_kind_ol, span: span}
        }
    }
}
fn matchify_if_instr(instr_ol: IfInstr, span: Span) -> Instr {
    let IfInstr {
        exp,
        iter_exps,
        block,
    } = instr_ol;
    let exp = matchify_exp(exp);
    let block = matchify_block(block);
    crate::phrase! {node: InstrKind::If(IfInstr {exp, iter_exps, block}), span: span}
}
fn matchify_hold_instr(instr_ol: HoldInstr, span: Span) -> Instr {
    let HoldInstr {
        id,
        not_exp,
        iter_exps,
        block_hold,
        block_not_hold,
    } = instr_ol;
    let block_hold = matchify_block(block_hold);
    let block_not_hold = matchify_block(block_not_hold);
    crate::phrase! {node: InstrKind::Hold(HoldInstr {id, not_exp, iter_exps, block_hold, block_not_hold}), span: span}
}
fn matchify_case_instr(instr_ol: CaseInstr, span: Span) -> Instr {
    let CaseInstr { exp, cases, total } = instr_ol;
    let cases = cases
        .into_iter()
        .map(|case| {
            let Case { guard, block } = case;
            let block = matchify_block(block);
            Case { guard, block }
        })
        .collect::<Vec<_>>();
    crate::phrase! {node: InstrKind::Case(CaseInstr {exp, cases, total}), span: span}
}
fn matchify_group_instr(instr_ol: GroupInstr, span: Span) -> Instr {
    let GroupInstr {
        id,
        rel_signature,
        exps,
        block,
    } = instr_ol;
    let block = matchify_block(block);
    crate::phrase! {node: InstrKind::Group(GroupInstr {id, rel_signature, exps, block}), span: span}
}
fn matchify_let_instr(instr_ol: LetInstr, span: Span) -> Instr {
    let LetInstr {
        exp_l,
        exp_r,
        iter_instrs,
        block,
    } = instr_ol;
    let block = matchify_block(block);
    crate::phrase! {node: InstrKind::Let(LetInstr {exp_l, exp_r, iter_instrs, block}), span: span}
}
fn matchify_rule_instr(instr_ol: RuleInstr, span: Span) -> Instr {
    let RuleInstr {
        id,
        not_exp,
        input_hint,
        iter_instrs,
        block,
    } = instr_ol;
    let block = matchify_block(block);
    crate::phrase! {node: InstrKind::Rule(RuleInstr {id, not_exp, input_hint, iter_instrs, block}), span: span}
}
fn matchify_block(block: Block) -> Block {
    block.into_iter().map(matchify_instr).collect()
}
pub(crate) fn apply(block: Block) -> Block {
    matchify_block(block)
}
