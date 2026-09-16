//! Remove unused OL Let bindings based on downstream uses
//!
//! `downstream_block` finds uses of the bound names before recursive cleanup;
//! `upstream_let_instr` drops a removable binding when none of them is used:
//!
//! ```text
//! let x = 1 { return y }
//!
//! becomes
//!
//! return y
//! ```
//!
//! `let x = f() { return y }` remains because its right-hand side is a call
//! Relation invocations also remain even when their outputs are unused

use crate::lang::{
    common::{ds::set::IdSet, source::Span},
    hints::input,
    il::ast::ExpKind,
    traits::free::Free,
};
use crate::pass::structure::{StructureError, StructureErrorKind, ol::ast::*};

// == Removable expressions

fn removable_let(exp_r: &Exp) -> bool {
    match &exp_r.node {
        ExpKind::Bool(_) | ExpKind::Num(_) | ExpKind::Text(_) | ExpKind::Var(_) => true,
        ExpKind::Un(_, _, exp)
        | ExpKind::UpCast(_, exp)
        | ExpKind::DownCast(_, exp)
        | ExpKind::Sub(exp, _, _)
        | ExpKind::Match(exp, _)
        | ExpKind::Len(exp)
        | ExpKind::Dot(exp, _)
        | ExpKind::Iter(exp, _) => removable_let(exp),
        ExpKind::Bin(_, _, exp_l, exp_r)
        | ExpKind::Cmp(_, _, exp_l, exp_r)
        | ExpKind::Cons(exp_l, exp_r)
        | ExpKind::Cat(exp_l, exp_r)
        | ExpKind::Mem(exp_l, exp_r)
        | ExpKind::Idx(exp_l, exp_r)
        | ExpKind::Upd(exp_l, _, exp_r) => removable_let(exp_l) && removable_let(exp_r),
        ExpKind::Tuple(exps) | ExpKind::List(exps) => exps.iter().all(removable_let),
        ExpKind::Case(not_exp) => not_exp.args().into_iter().all(removable_let),
        ExpKind::Str(exp_fields) => exp_fields.iter().all(|(_, exp)| removable_let(exp)),
        ExpKind::Opt(exp) => exp.as_deref().is_none_or(removable_let),
        ExpKind::Slice(exp_base, exp_idx, exp_len) => {
            removable_let(exp_base) && removable_let(exp_idx) && removable_let(exp_len)
        }
        ExpKind::Call(_, _, _) => false,
    }
}

// == Downstream uses

// - Checked rule inputs and outputs

fn split_rule<'a>(
    instr_ol: &'a RuleInstr,
    span: &Span,
) -> Result<(Vec<&'a Exp>, Vec<&'a Exp>), StructureError> {
    let RuleInstr {
        not_exp,
        input_hint,
        ..
    } = instr_ol;
    input::split(input_hint, not_exp.args())
        .map_err(|error| StructureError::new(StructureErrorKind::Input(error), span.clone()))
}

// - Instruction uses

fn downstream_instr(ids_defined: &IdSet, instr_ol: &Instr) -> Result<IdSet, StructureError> {
    downstream_instr_kind(ids_defined, &instr_ol.node, &instr_ol.span)
}

fn downstream_instr_kind(
    ids_defined: &IdSet,
    instr_kind_ol: &InstrKind,
    span: &Span,
) -> Result<IdSet, StructureError> {
    match instr_kind_ol {
        InstrKind::If(instr_ol) => downstream_if_instr(ids_defined, instr_ol),
        InstrKind::Hold(instr_ol) => downstream_hold_instr(ids_defined, instr_ol),
        InstrKind::Case(instr_ol) => downstream_case_instr(ids_defined, instr_ol),
        InstrKind::Group(instr_ol) => downstream_group_instr(ids_defined, instr_ol),
        InstrKind::Let(instr_ol) => downstream_let_instr(ids_defined, instr_ol),
        InstrKind::Rule(instr_ol) => downstream_rule_instr(ids_defined, instr_ol, span),
        InstrKind::Result(instr_ol) => downstream_result_instr(ids_defined, instr_ol),
        InstrKind::Return(instr_ol) => downstream_return_instr(ids_defined, instr_ol),
        InstrKind::Debug(instr_ol) => downstream_debug_instr(ids_defined, instr_ol),
    }
}

fn downstream_if_instr(ids_defined: &IdSet, instr_ol: &IfInstr) -> Result<IdSet, StructureError> {
    let IfInstr { exp, block, .. } = instr_ol;
    let ids_used = exp.free().intersection(ids_defined);
    let ids_used_then = downstream_block(ids_defined, block)?;
    Ok(ids_used.union(ids_used_then))
}

fn downstream_hold_instr(
    ids_defined: &IdSet,
    instr_ol: &HoldInstr,
) -> Result<IdSet, StructureError> {
    let HoldInstr {
        not_exp,
        block_hold,
        block_not_hold,
        ..
    } = instr_ol;
    let ids_used = not_exp.free().intersection(ids_defined);
    let ids_used_hold = downstream_block(ids_defined, block_hold)?;
    let ids_used_not_hold = downstream_block(ids_defined, block_not_hold)?;
    Ok(ids_used.union(ids_used_hold).union(ids_used_not_hold))
}

fn downstream_case_instr(
    ids_defined: &IdSet,
    instr_ol: &CaseInstr,
) -> Result<IdSet, StructureError> {
    let CaseInstr { exp, cases, .. } = instr_ol;
    let mut ids_used = exp.free().intersection(ids_defined);
    for case in cases {
        let Case { guard, block } = case;
        ids_used.append(guard.free().intersection(ids_defined));
        ids_used.append(downstream_block(ids_defined, block)?);
    }
    Ok(ids_used)
}

fn downstream_group_instr(
    ids_defined: &IdSet,
    instr_ol: &GroupInstr,
) -> Result<IdSet, StructureError> {
    let GroupInstr { exps, block, .. } = instr_ol;
    let ids_used = exps.free().intersection(ids_defined);
    let ids_used_block = downstream_block(ids_defined, block)?;
    Ok(ids_used.union(ids_used_block))
}

fn downstream_let_instr(ids_defined: &IdSet, instr_ol: &LetInstr) -> Result<IdSet, StructureError> {
    let LetInstr { exp_r, block, .. } = instr_ol;
    let ids_used = exp_r.free().intersection(ids_defined);
    let ids_used_block = downstream_block(ids_defined, block)?;
    Ok(ids_used.union(ids_used_block))
}

fn downstream_rule_instr(
    ids_defined: &IdSet,
    instr_ol: &RuleInstr,
    span: &Span,
) -> Result<IdSet, StructureError> {
    let (exps_input, _) = split_rule(instr_ol, span)?;
    let RuleInstr { block, .. } = instr_ol;
    let mut ids_input = IdSet::new();
    for exp in exps_input {
        exp.free_into(&mut ids_input);
    }
    let ids_used = ids_input.intersection(ids_defined);
    let ids_used_block = downstream_block(ids_defined, block)?;
    Ok(ids_used.union(ids_used_block))
}

fn downstream_result_instr(
    ids_defined: &IdSet,
    instr_ol: &ResultInstr,
) -> Result<IdSet, StructureError> {
    let ResultInstr { exps, .. } = instr_ol;
    Ok(exps.free().intersection(ids_defined))
}

fn downstream_return_instr(
    ids_defined: &IdSet,
    instr_ol: &ReturnInstr,
) -> Result<IdSet, StructureError> {
    let ReturnInstr { exp } = instr_ol;
    Ok(exp.free().intersection(ids_defined))
}

fn downstream_debug_instr(
    ids_defined: &IdSet,
    instr_ol: &DebugInstr,
) -> Result<IdSet, StructureError> {
    let DebugInstr { exp, instr } = instr_ol;
    let ids_used = exp.free().intersection(ids_defined);
    let ids_used_instr = downstream_instr(ids_defined, instr)?;
    Ok(ids_used.union(ids_used_instr))
}

fn downstream_block(ids_defined: &IdSet, block: &Block) -> Result<IdSet, StructureError> {
    let mut ids_defined = ids_defined.clone();
    let mut ids_used = IdSet::new();
    for instr_ol in block {
        ids_used.append(downstream_instr(&ids_defined, instr_ol)?);
        ids_defined = exclude_defined(ids_defined, &instr_ol.node, &instr_ol.span)?;
    }
    Ok(ids_used)
}

// - Definitions shadowing earlier bindings

fn exclude_defined(
    ids_defined: IdSet,
    instr_kind_ol: &InstrKind,
    span: &Span,
) -> Result<IdSet, StructureError> {
    match instr_kind_ol {
        InstrKind::Let(instr_ol) => exclude_let_defined(ids_defined, instr_ol),
        InstrKind::Rule(instr_ol) => exclude_rule_defined(ids_defined, instr_ol, span),
        _ => Ok(ids_defined),
    }
}

fn exclude_let_defined(ids_defined: IdSet, instr_ol: &LetInstr) -> Result<IdSet, StructureError> {
    let LetInstr { exp_l, .. } = instr_ol;
    Ok(ids_defined.difference(&exp_l.free()))
}

fn exclude_rule_defined(
    ids_defined: IdSet,
    instr_ol: &RuleInstr,
    span: &Span,
) -> Result<IdSet, StructureError> {
    let (_, exps_output) = split_rule(instr_ol, span)?;
    let mut ids_output = IdSet::new();
    for exp in exps_output {
        exp.free_into(&mut ids_output);
    }
    Ok(ids_defined.difference(&ids_output))
}

// == Binding removal

fn upstream_instr(instr_ol: Instr) -> Result<Block, StructureError> {
    upstream_instr_kind(instr_ol.node, instr_ol.span)
}

fn upstream_instr_kind(instr_kind_ol: InstrKind, span: Span) -> Result<Block, StructureError> {
    match instr_kind_ol {
        InstrKind::If(instr_ol) => upstream_if_instr(instr_ol, span),
        InstrKind::Hold(instr_ol) => upstream_hold_instr(instr_ol, span),
        InstrKind::Case(instr_ol) => upstream_case_instr(instr_ol, span),
        InstrKind::Group(instr_ol) => upstream_group_instr(instr_ol, span),
        InstrKind::Let(instr_ol) => upstream_let_instr(instr_ol, span),
        InstrKind::Rule(instr_ol) => upstream_rule_instr(instr_ol, span),
        InstrKind::Result(_) | InstrKind::Return(_) | InstrKind::Debug(_) => {
            Ok(vec![crate::phrase!(node: instr_kind_ol, span: span)])
        }
    }
}

fn upstream_if_instr(instr_ol: IfInstr, span: Span) -> Result<Block, StructureError> {
    let IfInstr {
        exp,
        iter_exps,
        block,
    } = instr_ol;
    let block = upstream_block(block)?;
    Ok(vec![
        crate::phrase!(node: InstrKind::If(IfInstr { exp, iter_exps, block }), span: span),
    ])
}

fn upstream_hold_instr(instr_ol: HoldInstr, span: Span) -> Result<Block, StructureError> {
    let HoldInstr {
        id,
        not_exp,
        iter_exps,
        block_hold,
        block_not_hold,
    } = instr_ol;
    let block_hold = upstream_block(block_hold)?;
    let block_not_hold = upstream_block(block_not_hold)?;
    Ok(vec![
        crate::phrase!(node: InstrKind::Hold(HoldInstr { id, not_exp, iter_exps, block_hold, block_not_hold }), span: span),
    ])
}

fn upstream_case_instr(instr_ol: CaseInstr, span: Span) -> Result<Block, StructureError> {
    let CaseInstr { exp, cases, total } = instr_ol;
    let cases = cases
        .into_iter()
        .map(|case| {
            let Case { guard, block } = case;
            let block = upstream_block(block)?;
            Ok(Case { guard, block })
        })
        .collect::<Result<_, StructureError>>()?;
    Ok(vec![
        crate::phrase!(node: InstrKind::Case(CaseInstr { exp, cases, total }), span: span),
    ])
}

fn upstream_group_instr(instr_ol: GroupInstr, span: Span) -> Result<Block, StructureError> {
    let GroupInstr {
        id,
        rel_signature,
        exps,
        block,
    } = instr_ol;
    let block = upstream_block(block)?;
    Ok(vec![
        crate::phrase!(node: InstrKind::Group(GroupInstr { id, rel_signature, exps, block }), span: span),
    ])
}

fn upstream_let_instr(instr_ol: LetInstr, span: Span) -> Result<Block, StructureError> {
    let LetInstr {
        exp_l,
        exp_r,
        iter_instrs,
        block,
    } = instr_ol;
    if removable_let(&exp_r) && downstream_block(&exp_l.free(), &block)?.is_empty() {
        return upstream_block(block);
    }
    let block = upstream_block(block)?;
    Ok(vec![
        crate::phrase!(node: InstrKind::Let(LetInstr { exp_l, exp_r, iter_instrs, block }), span: span),
    ])
}

fn upstream_rule_instr(instr_ol: RuleInstr, span: Span) -> Result<Block, StructureError> {
    let RuleInstr {
        id,
        not_exp,
        input_hint,
        iter_instrs,
        block,
    } = instr_ol;
    let block = upstream_block(block)?;
    Ok(vec![
        crate::phrase!(node: InstrKind::Rule(RuleInstr { id, not_exp, input_hint, iter_instrs, block }), span: span),
    ])
}

fn upstream_block(block: Block) -> Result<Block, StructureError> {
    let mut block_rewritten = Vec::new();
    for instr_ol in block {
        block_rewritten.extend(upstream_instr(instr_ol)?);
    }
    Ok(block_rewritten)
}

// == Entry point

pub(crate) fn apply(block: Block) -> Result<Block, StructureError> {
    upstream_block(block)
}
