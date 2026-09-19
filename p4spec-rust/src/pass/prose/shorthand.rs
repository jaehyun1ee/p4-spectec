//! Recognize compact prose instructions after annotation

use crate::lang::{
    il::ast::OptPattern,
    pl::{annot::Annotated, ast as pl},
};

fn eq_exp_var(exp_a: &pl::Exp, exp_b: &pl::Exp) -> bool {
    match (&exp_a.node.node, &exp_b.node.node) {
        (pl::ExpKind::Var(id_a), pl::ExpKind::Var(id_b)) => id_a.node == id_b.node,
        (pl::ExpKind::Iter(exp_a, _), pl::ExpKind::Iter(exp_b, _)) => eq_exp_var(exp_a, exp_b),
        _ => false,
    }
}

fn is_scrutinee_alias(exp_scrut: &pl::Exp, exp_r: &pl::Exp) -> bool {
    match &exp_r.node.node {
        pl::ExpKind::DownCast(_, exp_inner) => eq_exp_var(exp_scrut, exp_inner),
        _ => eq_exp_var(exp_scrut, exp_r),
    }
}

fn has_leading_rename<Tier>(exp_scrut: &pl::Exp, block: &pl::Block<Tier>) -> bool {
    matches!(
        block.first(),
        Some(instr) if matches!(
            &instr.node.node,
            pl::InstrKind::Let(pl::LetInstr { exp_r, iter_instrs, .. })
                if iter_instrs.is_empty() && is_scrutinee_alias(exp_scrut, exp_r)
        )
    )
}

fn take_leading_target<Tier>(block: &mut pl::Block<Tier>) -> pl::Exp {
    let instr = block.remove(0);
    let pl::InstrKind::Let(pl::LetInstr { exp_l, .. }) = instr.node.node else {
        unreachable!("leading rename was checked before consumption");
    };
    exp_l
}

fn shorten_case_guards<Tier>(instr: &mut pl::Instr<Tier>) {
    let pl::InstrKind::Case(pl::CaseInstr { exp, cases, .. }) = &mut instr.node.node else {
        return;
    };
    for case in cases {
        if !has_leading_rename(exp, &case.block) {
            continue;
        }
        let guard = match &case.guard {
            pl::Guard::Sub(typ, subcheck) => {
                let exp_target = take_leading_target(&mut case.block);
                Some(pl::Guard::CheckLetSub(typ.clone(), subcheck.clone(), exp_target))
            }
            pl::Guard::Match(pattern) => {
                let exp_target = take_leading_target(&mut case.block);
                Some(pl::Guard::CheckLetMatch(pattern.clone(), exp_target))
            }
            _ => None,
        };
        if let Some(guard) = guard {
            case.guard = guard;
        }
    }
}

fn shorten_check_let<Tier>(instr: &mut pl::Instr<Tier>) {
    enum Check {
        Sub(pl::Typ, Box<pl::Subcheck>, pl::Exp),
        Match(pl::Pattern, pl::Exp),
    }

    let check = match &instr.node.node {
        pl::InstrKind::If(pl::IfInstr { exp, iter_exps, block, .. })
            if iter_exps.is_empty() && has_leading_rename(exp, block) =>
        {
            match &exp.node.node {
                pl::ExpKind::Sub(exp_scrut, typ, subcheck) => {
                    Some(Check::Sub(typ.clone(), subcheck.clone(), exp_scrut.as_ref().clone()))
                }
                pl::ExpKind::Match(exp_scrut, pattern) => {
                    Some(Check::Match(pattern.clone(), exp_scrut.as_ref().clone()))
                }
                _ => None,
            }
        }
        pl::InstrKind::Case(pl::CaseInstr { exp, cases, .. }) if cases.len() == 1 => {
            let case = &cases[0];
            if !has_leading_rename(exp, &case.block) {
                return;
            }
            match &case.guard {
                pl::Guard::Sub(typ, subcheck) => {
                    Some(Check::Sub(typ.clone(), subcheck.clone(), exp.clone()))
                }
                pl::Guard::Match(pattern) => Some(Check::Match(pattern.clone(), exp.clone())),
                _ => None,
            }
        }
        _ => None,
    };
    let Some(check) = check else { return };

    let block = match &mut instr.node.node {
        pl::InstrKind::If(pl::IfInstr { block, .. }) => block,
        pl::InstrKind::Case(pl::CaseInstr { cases, .. }) => &mut cases[0].block,
        _ => unreachable!(),
    };
    let exp_l = take_leading_target(block);
    let block = std::mem::take(block);
    instr.node.node = match check {
        Check::Sub(typ, subcheck, exp_r) => {
            pl::InstrKind::CheckLetSub(pl::CheckLetSubInstr { typ, subcheck, exp_l, exp_r, block })
        }
        Check::Match(pattern, exp_r) => {
            pl::InstrKind::CheckLetMatch(pl::CheckLetMatchInstr { pattern, exp_l, exp_r, block })
        }
    };
}

fn visible(exp: &pl::Exp) -> bool {
    match &exp.node.node {
        pl::ExpKind::Var(id) => !id.node.starts_with('_'),
        pl::ExpKind::Iter(exp_inner, _) => visible(exp_inner),
        _ => true,
    }
}

fn shorten_destruct<Tier>(instr: &mut pl::Instr<Tier>) {
    let Some(field_names) = instr.hints.prose_fields.as_ref().map(|hint| hint.fields()) else {
        return;
    };
    let pl::InstrKind::Let(pl::LetInstr { exp_l, exp_r, iter_instrs }) = &instr.node.node else {
        return;
    };
    if !iter_instrs.is_empty() {
        return;
    }
    let pl::ExpKind::Case(not_exp) = &exp_l.node.node else {
        return;
    };
    let exps = not_exp.args();
    if exps.len() != field_names.len() || exps.iter().all(|exp| !visible(exp)) {
        return;
    }
    let bindings = exps
        .into_iter()
        .zip(field_names)
        .map(|(exp, name)| (visible(exp).then(|| name.clone()), exp.clone()))
        .collect();
    instr.node.node = pl::InstrKind::Destruct(pl::DestructInstr { bindings, exp: exp_r.clone() });
}

fn shorten_single<Tier>(mut instr: pl::Instr<Tier>) -> pl::Instr<Tier> {
    shorten_case_guards(&mut instr);
    shorten_check_let(&mut instr);
    shorten_destruct(&mut instr);
    instr
}

fn shorten_option_get<Tier>(block: &mut pl::Block<Tier>) -> Option<pl::Instr<Tier>> {
    if block.len() < 2 {
        return None;
    }
    let is_option_get = match (&block[0].node.node, &block[1].node.node) {
        (
            pl::InstrKind::Let(pl::LetInstr {
                exp_l: exp_tmp,
                exp_r: exp_value,
                iter_instrs: iter_let,
            }),
            pl::InstrKind::If(pl::IfInstr {
                exp: exp_cond,
                iter_exps: iter_if,
                block: block_then,
                ..
            }),
        ) if iter_let.is_empty() && iter_if.is_empty() => {
            let pl::ExpKind::Match(exp_scrut, pl::Pattern::Opt(OptPattern::Some)) =
                &exp_cond.node.node
            else {
                return None;
            };
            let instr_then = block_then.first()?;
            let pl::InstrKind::Let(pl::LetInstr { exp_l, exp_r, iter_instrs }) =
                &instr_then.node.node
            else {
                return None;
            };
            let pl::ExpKind::Opt(Some(exp_target)) = &exp_l.node.node else {
                return None;
            };
            if !iter_instrs.is_empty()
                || !eq_exp_var(exp_tmp, exp_scrut)
                || !eq_exp_var(exp_tmp, exp_r)
            {
                return None;
            }
            let _ = (exp_target, exp_value);
            true
        }
        _ => false,
    };
    if !is_option_get {
        return None;
    }
    let instr_source = block.remove(0);
    let instr_if = block.remove(0);
    let pl::InstrKind::Let(pl::LetInstr { exp_r: exp_value, .. }) = instr_source.node.node else {
        unreachable!();
    };
    let pl::InstrKind::If(pl::IfInstr { mut block, .. }) = instr_if.node.node else {
        unreachable!();
    };
    let instr_target = block.remove(0);
    let pl::InstrKind::Let(pl::LetInstr { exp_l, .. }) = instr_target.node.node else {
        unreachable!();
    };
    let pl::ExpKind::Opt(Some(exp_target)) = exp_l.node.node else {
        unreachable!();
    };
    Some(Annotated {
        node: crate::note_phrase! {
            node: pl::InstrKind::OptionGet(pl::OptionGetInstr {
                exp_l: *exp_target,
                exp_r: exp_value,
                block,
            }),
            note: instr_source.node.note,
            span: instr_source.node.span,
        },
        hints: instr_source.hints,
    })
}

trait ShortenTier: Sized {
    fn shorten(self) -> Self;
}

fn shorten_hold_case<Tier: ShortenTier>(hold_case: pl::HoldCase<Tier>) -> pl::HoldCase<Tier> {
    match hold_case {
        pl::HoldCase::Both(block_hold, block_not_hold) => {
            pl::HoldCase::Both(shorten_block(block_hold), shorten_block(block_not_hold))
        }
        pl::HoldCase::Hold(block, dangle) => pl::HoldCase::Hold(shorten_block(block), dangle),
        pl::HoldCase::NotHold(block, dangle) => pl::HoldCase::NotHold(shorten_block(block), dangle),
    }
}

fn shorten_recurse<Tier: ShortenTier>(mut instr: pl::Instr<Tier>) -> pl::Instr<Tier> {
    instr.node.node = match instr.node.node {
        pl::InstrKind::If(mut instr_if) => {
            instr_if.block = shorten_block(instr_if.block);
            pl::InstrKind::If(instr_if)
        }
        pl::InstrKind::Hold(mut instr_hold) => {
            instr_hold.hold_case = shorten_hold_case(instr_hold.hold_case);
            pl::InstrKind::Hold(instr_hold)
        }
        pl::InstrKind::Case(mut instr_case) => {
            for case in &mut instr_case.cases {
                case.block = shorten_block(std::mem::take(&mut case.block));
            }
            pl::InstrKind::Case(instr_case)
        }
        pl::InstrKind::CheckLetSub(mut instr_check) => {
            instr_check.block = shorten_block(instr_check.block);
            pl::InstrKind::CheckLetSub(instr_check)
        }
        pl::InstrKind::CheckLetMatch(mut instr_check) => {
            instr_check.block = shorten_block(instr_check.block);
            pl::InstrKind::CheckLetMatch(instr_check)
        }
        pl::InstrKind::OptionGet(mut instr_get) => {
            instr_get.block = shorten_block(instr_get.block);
            pl::InstrKind::OptionGet(instr_get)
        }
        pl::InstrKind::Tier(pl::TierInstr { tier }) => {
            pl::InstrKind::Tier(pl::TierInstr { tier: tier.shorten() })
        }
        kind @ (pl::InstrKind::Let(_) | pl::InstrKind::Debug(_) | pl::InstrKind::Destruct(_)) => {
            kind
        }
    };
    instr
}

fn shorten_block<Tier: ShortenTier>(mut block: pl::Block<Tier>) -> pl::Block<Tier> {
    let mut block_output = Vec::new();
    while !block.is_empty() {
        if let Some(instr) = shorten_option_get(&mut block) {
            block_output.push(instr);
        } else {
            block_output.push(block.remove(0));
        }
    }
    block_output
        .into_iter()
        .map(shorten_single)
        .map(shorten_recurse)
        .collect()
}

impl ShortenTier for pl::InstrDispatch {
    fn shorten(self) -> Self {
        match self {
            Self::Group(mut instr_group) => {
                instr_group.block = shorten_block(instr_group.block);
                Self::Group(instr_group)
            }
            Self::Route(mut instr_route) => {
                instr_route.blocks = instr_route.blocks.into_iter().map(shorten_block).collect();
                Self::Route(instr_route)
            }
        }
    }
}

impl ShortenTier for pl::InstrGroup {
    fn shorten(self) -> Self {
        match self {
            Self::Backtrack(mut instr_backtrack) => {
                instr_backtrack.blocks = instr_backtrack
                    .blocks
                    .into_iter()
                    .map(shorten_block)
                    .collect();
                Self::Backtrack(instr_backtrack)
            }
            instr @ (Self::Result(_) | Self::Return(_) | Self::Rule(_)) => instr,
        }
    }
}

fn shorten_def(mut def: pl::Def) -> pl::Def {
    def.node.node = match def.node.node {
        pl::DefKind::Rel(pl::RelDef::Defined(mut def_rel)) => {
            def_rel.block = shorten_block(def_rel.block);
            def_rel.block_else_opt = def_rel.block_else_opt.map(shorten_block);
            pl::DefKind::Rel(pl::RelDef::Defined(def_rel))
        }
        pl::DefKind::MetaFunc(pl::MetaFuncDef::Table(mut def_func)) => {
            for row in &mut def_func.rows {
                row.block = shorten_block(std::mem::take(&mut row.block));
            }
            pl::DefKind::MetaFunc(pl::MetaFuncDef::Table(def_func))
        }
        pl::DefKind::MetaFunc(pl::MetaFuncDef::Defined(mut def_func)) => {
            def_func.block = shorten_block(def_func.block);
            def_func.block_else_opt = def_func.block_else_opt.map(shorten_block);
            pl::DefKind::MetaFunc(pl::MetaFuncDef::Defined(def_func))
        }
        kind => kind,
    };
    def
}

pub(super) fn spec(spec: pl::Spec) -> pl::Spec {
    spec.into_iter().map(shorten_def).collect()
}
