//! Collapse adjacent equivalent bindings with capture-avoiding body renaming
use crate::lang::{
    common::source::{Phrase, Span},
    hints::input,
    il::ast::{ExpField, ExpKind},
    traits::{eq::SyntaxEq, free::Free},
};
use crate::pass::structure::{
    StructureError, StructureErrorKind, merge::merge_block, ol::ast::*, re::renamer::Renamer,
};

struct ExpUnit<'a> {
    exp: &'a Exp,
    iter_exps: Vec<ExpIter>,
}
impl SyntaxEq for ExpUnit<'_> {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.exp.syntax_eq(other.exp) && self.iter_exps.syntax_eq(&other.iter_exps)
    }
}
enum Bind<'a> {
    Let(ExpUnit<'a>, ExpUnit<'a>),
    Rule(&'a Id, Vec<ExpUnit<'a>>, Vec<ExpUnit<'a>>),
}
fn init_expunit<'a>(exp: &'a Exp, iter_exps: &[ExpIter]) -> ExpUnit<'a> {
    let ids = exp.free();
    let iter_exps = iter_exps
        .iter()
        .map(|(iter, vars)| {
            let vars = vars
                .iter()
                .filter(|var| ids.contains(&var.id))
                .cloned()
                .collect();
            (*iter, vars)
        })
        .collect();
    ExpUnit { exp, iter_exps }
}
fn split_iterinstrs(iter_instrs: &[InstrIter]) -> (Vec<ExpIter>, Vec<ExpIter>) {
    iter_instrs
        .iter()
        .map(|iter_instr| {
            let InstrIter {
                iter,
                vars_bound,
                vars_bind,
            } = iter_instr;
            ((*iter, vars_bound.clone()), (*iter, vars_bind.clone()))
        })
        .unzip()
}
fn init_let_bind(instr_let: &LetInstr) -> Bind<'_> {
    let LetInstr {
        exp_l,
        exp_r,
        iter_instrs,
        ..
    } = instr_let;
    let (iter_exps_bound, iter_exps_bind) = split_iterinstrs(iter_instrs);
    Bind::Let(
        init_expunit(exp_l, &iter_exps_bind),
        init_expunit(exp_r, &iter_exps_bound),
    )
}
fn init_rule_bind<'a>(instr_rule: &'a RuleInstr, span: &Span) -> Result<Bind<'a>, StructureError> {
    let RuleInstr {
        id,
        not_exp,
        input_hint,
        iter_instrs,
        ..
    } = instr_rule;
    let (exps_input, exps_output) = input::split(input_hint, not_exp.args())
        .map_err(|error| StructureError::new(StructureErrorKind::Input(error), span.clone()))?;
    let (iter_exps_bound, iter_exps_bind) = split_iterinstrs(iter_instrs);
    let expunits_input = exps_input
        .into_iter()
        .map(|exp| init_expunit(exp, &iter_exps_bound))
        .collect();
    let expunits_output = exps_output
        .into_iter()
        .map(|exp| init_expunit(exp, &iter_exps_bind))
        .collect();
    Ok(Bind::Rule(id, expunits_input, expunits_output))
}
fn collapse_exp(renamer: Renamer, exp: &Exp, exp_target: &Exp) -> Option<Renamer> {
    let exp_kind = &exp.node;
    let exp_kind_target = &exp_target.node;
    match (exp_kind, exp_kind_target) {
        (ExpKind::Var(id), ExpKind::Var(id_target)) => collapse_var_exp(renamer, id, id_target),
        (ExpKind::Tuple(exps), ExpKind::Tuple(exps_target))
        | (ExpKind::List(exps), ExpKind::List(exps_target)) => {
            collapse_exps(renamer, exps, exps_target)
        }
        (ExpKind::Case(not_exp), ExpKind::Case(not_exp_target)) => {
            collapse_case_exp(renamer, not_exp, not_exp_target)
        }
        (ExpKind::Str(exp_fields), ExpKind::Str(exp_fields_target)) => {
            collapse_str_exp(renamer, exp_fields, exp_fields_target)
        }
        (ExpKind::Opt(exp), ExpKind::Opt(exp_target)) => {
            collapse_opt_exp(renamer, exp.as_deref(), exp_target.as_deref())
        }
        (ExpKind::Cons(exp_head, exp_tail), ExpKind::Cons(exp_head_target, exp_tail_target)) => {
            collapse_cons_exp(
                renamer,
                exp_head,
                exp_tail,
                exp_head_target,
                exp_tail_target,
            )
        }
        (ExpKind::Iter(exp, iter_exp), ExpKind::Iter(exp_target, iter_exp_target)) => {
            collapse_iter_exp(renamer, exp, iter_exp, exp_target, iter_exp_target)
        }
        _ => None,
    }
}
fn collapse_var_exp(mut renamer: Renamer, id: &Id, id_target: &Id) -> Option<Renamer> {
    if !id.syntax_eq(id_target) {
        renamer.add(id_target.clone(), id.clone());
    }
    Some(renamer)
}
fn collapse_case_exp(
    renamer: Renamer,
    not_exp: &NotExp,
    not_exp_target: &NotExp,
) -> Option<Renamer> {
    if !not_exp.to_mixop().syntax_eq(&not_exp_target.to_mixop()) {
        return None;
    }
    collapse_exp_refs(renamer, not_exp.args(), not_exp_target.args())
}
fn collapse_str_exp(
    renamer: Renamer,
    exp_fields: &[ExpField],
    exp_fields_target: &[ExpField],
) -> Option<Renamer> {
    if exp_fields.len() != exp_fields_target.len()
        || !exp_fields
            .iter()
            .zip(exp_fields_target)
            .all(|((atom, _), (atom_target, _))| atom.syntax_eq(atom_target))
    {
        return None;
    }
    collapse_exp_refs(
        renamer,
        exp_fields.iter().map(|(_, exp)| exp).collect(),
        exp_fields_target.iter().map(|(_, exp)| exp).collect(),
    )
}
fn collapse_opt_exp(
    renamer: Renamer,
    exp: Option<&Exp>,
    exp_target: Option<&Exp>,
) -> Option<Renamer> {
    match (exp, exp_target) {
        (Some(exp), Some(exp_target)) => collapse_exp(renamer, exp, exp_target),
        (None, None) => Some(renamer),
        _ => None,
    }
}
fn collapse_cons_exp(
    renamer: Renamer,
    exp_head: &Exp,
    exp_tail: &Exp,
    exp_head_target: &Exp,
    exp_tail_target: &Exp,
) -> Option<Renamer> {
    let renamer = collapse_exp(renamer, exp_head, exp_head_target)?;
    collapse_exp(renamer, exp_tail, exp_tail_target)
}
fn collapse_iter_exp(
    renamer: Renamer,
    exp: &Exp,
    iter_exp: &ExpIter,
    exp_target: &Exp,
    iter_exp_target: &ExpIter,
) -> Option<Renamer> {
    let renamer = collapse_exp(renamer, exp, exp_target)?;
    let iter_exp_target = renamer.rename_iterexp(iter_exp_target.clone());
    iter_exp.syntax_eq(&iter_exp_target).then_some(renamer)
}
fn collapse_exps(renamer: Renamer, exps: &[Exp], exps_target: &[Exp]) -> Option<Renamer> {
    collapse_exp_refs(renamer, exps.iter().collect(), exps_target.iter().collect())
}
fn collapse_exp_refs(
    mut renamer: Renamer,
    exps: Vec<&Exp>,
    exps_target: Vec<&Exp>,
) -> Option<Renamer> {
    if exps.len() != exps_target.len() {
        return None;
    }
    for (exp, exp_target) in exps.into_iter().zip(exps_target) {
        renamer = collapse_exp(renamer, exp, exp_target)?;
    }
    Some(renamer)
}
fn collapse_expunit(
    renamer: Renamer,
    expunit: &ExpUnit<'_>,
    expunit_target: &ExpUnit<'_>,
) -> Option<Renamer> {
    let renamer = collapse_exp(renamer, expunit.exp, expunit_target.exp)?;
    let iter_exps_target = renamer.rename_iterexps(expunit_target.iter_exps.clone());
    expunit
        .iter_exps
        .syntax_eq(&iter_exps_target)
        .then_some(renamer)
}
fn collapse_expunits(
    mut renamer: Renamer,
    expunits: &[ExpUnit<'_>],
    expunits_target: &[ExpUnit<'_>],
) -> Option<Renamer> {
    if expunits.len() != expunits_target.len() {
        return None;
    }
    for (expunit, expunit_target) in expunits.iter().zip(expunits_target) {
        renamer = collapse_expunit(renamer, expunit, expunit_target)?;
    }
    Some(renamer)
}
fn collapse_bind(bind: &Bind<'_>, bind_target: &Bind<'_>) -> Option<Renamer> {
    match (bind, bind_target) {
        (Bind::Let(expunit_l, expunit_r), Bind::Let(expunit_target_l, expunit_target_r))
            if expunit_r.syntax_eq(expunit_target_r) =>
        {
            collapse_expunit(Renamer::empty(), expunit_l, expunit_target_l)
        }
        (
            Bind::Rule(id, expunits_input, expunits_output),
            Bind::Rule(id_target, expunits_target_input, expunits_target_output),
        ) if id.syntax_eq(id_target) && expunits_input.syntax_eq(expunits_target_input) => {
            collapse_expunits(Renamer::empty(), expunits_output, expunits_target_output)
        }
        _ => None,
    }
}
fn downstream(bind: &Bind<'_>, block: &mut Block) -> Result<Option<Block>, StructureError> {
    let Some(instr_head) = block.first_mut() else {
        return Ok(None);
    };
    let Phrase {
        node: instr_kind,
        span,
        ..
    } = instr_head;
    let block_merge = match instr_kind {
        InstrKind::Let(instr_let) => downstream_let_instr(bind, instr_let)?,
        InstrKind::Rule(instr_rule) => downstream_rule_instr(bind, instr_rule, span)?,
        _ => None,
    };
    if block_merge.is_some() {
        block.remove(0);
    }
    Ok(block_merge)
}
fn downstream_let_instr(
    bind: &Bind<'_>,
    instr_let: &mut LetInstr,
) -> Result<Option<Block>, StructureError> {
    let bind_target = init_let_bind(instr_let);
    let Some(renamer) = collapse_bind(bind, &bind_target) else {
        return Ok(None);
    };
    let LetInstr { block, .. } = instr_let;
    let block = renamer.rename_block(std::mem::take(block))?;
    Ok(Some(block))
}
fn downstream_rule_instr(
    bind: &Bind<'_>,
    instr_rule: &mut RuleInstr,
    span: &Span,
) -> Result<Option<Block>, StructureError> {
    let bind_target = init_rule_bind(instr_rule, span)?;
    let Some(renamer) = collapse_bind(bind, &bind_target) else {
        return Ok(None);
    };
    let RuleInstr { block, .. } = instr_rule;
    let block = renamer.rename_block(std::mem::take(block))?;
    Ok(Some(block))
}
fn upstream(block: Block) -> Result<Block, StructureError> {
    let mut instrs = block.into_iter();
    let Some(instr_head) = instrs.next() else {
        return Ok(vec![]);
    };
    let Phrase {
        node: instr_kind,
        span,
        note: (),
    } = instr_head;
    let block_tail = instrs.collect();
    match instr_kind {
        InstrKind::If(instr_if) => upstream_if_instr(instr_if, span, block_tail),
        InstrKind::Hold(instr_hold) => upstream_hold_instr(instr_hold, span, block_tail),
        InstrKind::Case(instr_case) => upstream_case_instr(instr_case, span, block_tail),
        InstrKind::Group(instr_group) => upstream_group_instr(instr_group, span, block_tail),
        InstrKind::Let(instr_let) => upstream_let_instr(instr_let, span, block_tail),
        InstrKind::Rule(instr_rule) => upstream_rule_instr(instr_rule, span, block_tail),
        instr_kind => finish(instr_kind, span, block_tail),
    }
}
fn finish(instr_kind: InstrKind, span: Span, block_tail: Block) -> Result<Block, StructureError> {
    let mut block = vec![Phrase {
        node: instr_kind,
        span,
        note: (),
    }];
    block.extend(upstream(block_tail)?);
    Ok(block)
}
fn upstream_if_instr(
    instr_if: IfInstr,
    span: Span,
    block_tail: Block,
) -> Result<Block, StructureError> {
    let IfInstr {
        exp,
        iter_exps,
        block,
    } = instr_if;
    let block = upstream(block)?;
    finish(
        InstrKind::If(IfInstr {
            exp,
            iter_exps,
            block,
        }),
        span,
        block_tail,
    )
}
fn upstream_hold_instr(
    instr_hold: HoldInstr,
    span: Span,
    block_tail: Block,
) -> Result<Block, StructureError> {
    let HoldInstr {
        id,
        not_exp,
        iter_exps,
        block_hold,
        block_not_hold,
    } = instr_hold;
    let block_hold = upstream(block_hold)?;
    let block_not_hold = upstream(block_not_hold)?;
    finish(
        InstrKind::Hold(HoldInstr {
            id,
            not_exp,
            iter_exps,
            block_hold,
            block_not_hold,
        }),
        span,
        block_tail,
    )
}
fn upstream_case_instr(
    instr_case: CaseInstr,
    span: Span,
    block_tail: Block,
) -> Result<Block, StructureError> {
    let CaseInstr { exp, cases, total } = instr_case;
    let cases = cases
        .into_iter()
        .map(|case| {
            let Case { guard, block } = case;
            let block = upstream(block)?;
            Ok(Case { guard, block })
        })
        .collect::<Result<_, StructureError>>()?;
    finish(
        InstrKind::Case(CaseInstr { exp, cases, total }),
        span,
        block_tail,
    )
}
fn upstream_group_instr(
    instr_group: GroupInstr,
    span: Span,
    block_tail: Block,
) -> Result<Block, StructureError> {
    let GroupInstr {
        id,
        rel_signature,
        exps,
        block,
    } = instr_group;
    let block = upstream(block)?;
    finish(
        InstrKind::Group(GroupInstr {
            id,
            rel_signature,
            exps,
            block,
        }),
        span,
        block_tail,
    )
}
fn upstream_let_instr(
    instr_let: LetInstr,
    span: Span,
    mut block_tail: Block,
) -> Result<Block, StructureError> {
    let bind = init_let_bind(&instr_let);
    let block_merge = downstream(&bind, &mut block_tail)?;
    let LetInstr {
        exp_l,
        exp_r,
        iter_instrs,
        block,
    } = instr_let;
    if let Some(block_merge) = block_merge {
        let block = merge_block(block, block_merge);
        let instr_kind = InstrKind::Let(LetInstr {
            exp_l,
            exp_r,
            iter_instrs,
            block,
        });
        let mut block = vec![Phrase {
            node: instr_kind,
            span,
            note: (),
        }];
        block.extend(block_tail);
        upstream(block)
    } else {
        let block = upstream(block)?;
        finish(
            InstrKind::Let(LetInstr {
                exp_l,
                exp_r,
                iter_instrs,
                block,
            }),
            span,
            block_tail,
        )
    }
}
fn upstream_rule_instr(
    instr_rule: RuleInstr,
    span: Span,
    mut block_tail: Block,
) -> Result<Block, StructureError> {
    let bind = init_rule_bind(&instr_rule, &span)?;
    let block_merge = downstream(&bind, &mut block_tail)?;
    let RuleInstr {
        id,
        not_exp,
        input_hint,
        iter_instrs,
        block,
    } = instr_rule;
    if let Some(block_merge) = block_merge {
        let block = merge_block(block, block_merge);
        let instr_kind = InstrKind::Rule(RuleInstr {
            id,
            not_exp,
            input_hint,
            iter_instrs,
            block,
        });
        let mut block = vec![Phrase {
            node: instr_kind,
            span,
            note: (),
        }];
        block.extend(block_tail);
        upstream(block)
    } else {
        let block = upstream(block)?;
        finish(
            InstrKind::Rule(RuleInstr {
                id,
                not_exp,
                input_hint,
                iter_instrs,
                block,
            }),
            span,
            block_tail,
        )
    }
}
pub(crate) fn apply(block: Block) -> Result<Block, StructureError> {
    upstream(block)
}
