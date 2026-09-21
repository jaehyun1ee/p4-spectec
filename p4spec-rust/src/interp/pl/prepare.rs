//! Runtime preparation of annotated PL expressions
//!
//! PL definitions remain intact in the execution environment. Expressions are
//! projected to the shared executable syntax only when evaluated; each
//! callable's layout is populated once while loading its definition.

use crate::{
    interp::shared::prepare::{Prepare, ast as exec},
    lang::{common::notation::mixfix::Mixfix, il, pl::ast as pl},
    runtime::envs::interp::shared::frame::FrameLayout,
};

fn lower_mixfix(mixfix: &pl::NotExp) -> il::ast::NotExp {
    match mixfix {
        Mixfix::Arg(exp) => Mixfix::Arg(lower_exp(exp)),
        Mixfix::Atom(atom) => Mixfix::Atom(atom.clone()),
        Mixfix::Brack(atom_l, inner, atom_r) => {
            Mixfix::Brack(atom_l.clone(), Box::new(lower_mixfix(inner)), atom_r.clone())
        }
        Mixfix::Infix(exp_l, atom, exp_r) => Mixfix::Infix(
            Box::new(lower_mixfix(exp_l)),
            atom.clone(),
            Box::new(lower_mixfix(exp_r)),
        ),
        Mixfix::Seq(items) => Mixfix::Seq(items.iter().map(lower_mixfix).collect()),
    }
}

fn lower_path(path: &pl::Path) -> il::ast::Path {
    let node = match &path.node {
        pl::PathKind::Root => il::ast::PathKind::Root,
        pl::PathKind::Idx(path, exp) => {
            il::ast::PathKind::Idx(Box::new(lower_path(path)), Box::new(lower_exp(exp)))
        }
        pl::PathKind::Slice(path, exp_idx, exp_len) => il::ast::PathKind::Slice(
            Box::new(lower_path(path)),
            Box::new(lower_exp(exp_idx)),
            Box::new(lower_exp(exp_len)),
        ),
        pl::PathKind::Dot(path, atom) => {
            il::ast::PathKind::Dot(Box::new(lower_path(path)), atom.clone())
        }
    };
    crate::note_phrase!(node: node, note: path.note.clone(), span: path.span.clone())
}

fn lower_arg(arg: &pl::Arg) -> il::ast::Arg {
    let node = match &arg.node {
        pl::ArgKind::Exp(exp) => il::ast::ArgKind::Exp(Box::new(lower_exp(exp))),
        pl::ArgKind::Def(id) => il::ast::ArgKind::Def(id.clone()),
    };
    crate::phrase!(node: node, span: arg.span.clone())
}

/// Removes prose annotations while retaining executable syntax and source data.
pub(crate) fn lower_exp(exp: &pl::Exp) -> il::ast::Exp {
    use il::ast::ExpKind as E;
    use pl::ExpKind as P;
    let node = match &exp.node.node {
        P::Bool(value) => E::Bool(*value),
        P::Num(num) => E::Num(num.clone()),
        P::Text(text) => E::Text(text.clone()),
        P::Id(id) => E::Id(id.clone()),
        P::Un(op, typ, exp) => E::Un(*op, *typ, Box::new(lower_exp(exp))),
        P::Bin(op, typ, exp_l, exp_r) => {
            E::Bin(*op, *typ, Box::new(lower_exp(exp_l)), Box::new(lower_exp(exp_r)))
        }
        P::Cmp(op, typ, exp_l, exp_r) => {
            E::Cmp(*op, *typ, Box::new(lower_exp(exp_l)), Box::new(lower_exp(exp_r)))
        }
        P::UpCast(typ, exp) => E::UpCast(Box::new(typ.clone()), Box::new(lower_exp(exp))),
        P::DownCast(typ, exp) => E::DownCast(Box::new(typ.clone()), Box::new(lower_exp(exp))),
        P::Sub(exp, typ, check) => {
            E::Sub(Box::new(lower_exp(exp)), Box::new(typ.clone()), check.clone())
        }
        P::Match(exp, pattern) => E::Match(Box::new(lower_exp(exp)), pattern.clone()),
        P::Tuple(exps) => E::Tuple(exps.iter().map(lower_exp).collect()),
        P::Case(not_exp) => E::Case(Box::new(lower_mixfix(not_exp))),
        P::Str(fields) => E::Str(
            fields
                .iter()
                .map(|(atom, exp)| il::ast::ExpField { atom: atom.clone(), exp: lower_exp(exp) })
                .collect(),
        ),
        P::Opt(exp) => E::Opt(exp.as_ref().map(|exp| Box::new(lower_exp(exp)))),
        P::List(exps) => E::List(exps.iter().map(lower_exp).collect()),
        P::Cons(exp_head, exp_tail) => {
            E::Cons(Box::new(lower_exp(exp_head)), Box::new(lower_exp(exp_tail)))
        }
        P::Cat(exp_l, exp_r) => E::Cat(Box::new(lower_exp(exp_l)), Box::new(lower_exp(exp_r))),
        P::Mem(exp_elem, exp_list) => {
            E::Mem(Box::new(lower_exp(exp_elem)), Box::new(lower_exp(exp_list)))
        }
        P::Len(exp) => E::Len(Box::new(lower_exp(exp))),
        P::Dot(exp, atom) => E::Dot(Box::new(lower_exp(exp)), atom.clone()),
        P::Idx(exp_base, exp_idx) => {
            E::Idx(Box::new(lower_exp(exp_base)), Box::new(lower_exp(exp_idx)))
        }
        P::Slice(exp_base, exp_idx, exp_len) => E::Slice(
            Box::new(lower_exp(exp_base)),
            Box::new(lower_exp(exp_idx)),
            Box::new(lower_exp(exp_len)),
        ),
        P::Upd(exp_base, path, exp_new) => E::Upd(
            Box::new(lower_exp(exp_base)),
            Box::new(lower_path(path)),
            Box::new(lower_exp(exp_new)),
        ),
        P::Call(id, targs, args) => {
            E::Call(id.clone(), targs.clone(), args.iter().map(lower_arg).collect())
        }
        P::Iter(exp, iter) => E::Iter(Box::new(lower_exp(exp)), iter.clone()),
    };
    crate::note_phrase! {
        node: node,
        note: exp.node.note.clone(),
        span: exp.node.span.clone(),
    }
}

pub(crate) fn prepare_exp(layout: &FrameLayout, exp: &pl::Exp) -> exec::Exp {
    let len = layout.len();
    let mut layout = layout.clone();
    let exp = lower_exp(exp).prepare(&mut layout);
    assert_eq!(layout.len(), len, "PL callable layout was not fully reserved");
    exp
}

pub(crate) fn prepare_exps(layout: &FrameLayout, exps: &[pl::Exp]) -> Vec<exec::Exp> {
    exps.iter().map(|exp| prepare_exp(layout, exp)).collect()
}

fn reserve_exp(layout: &mut FrameLayout, exp: &pl::Exp) {
    let _ = lower_exp(exp).prepare(layout);
}

fn reserve_param(layout: &mut FrameLayout, param: &pl::Param) {
    match &param.node {
        pl::ParamKind::Exp(_, exp) => reserve_exp(layout, exp),
        pl::ParamKind::Def(_, _, params, _) => {
            for param in params {
                reserve_param(layout, param);
            }
        }
    }
}

fn reserve_group_block(layout: &mut FrameLayout, block: &pl::GroupBlock) {
    for instr in block {
        if let pl::InstrKind::Tier(instr) = &instr.node.node {
            match &instr.tier {
                pl::GroupInstr::Result(instr) => {
                    for exp in &instr.exps_output {
                        reserve_exp(layout, exp);
                    }
                }
                pl::GroupInstr::Return(instr) => reserve_exp(layout, &instr.exp),
                pl::GroupInstr::Rule(instr) => {
                    for exp in instr.not_exp.args() {
                        reserve_exp(layout, exp);
                    }
                    let _ = instr.iter_instrs.clone().prepare(layout);
                }
                pl::GroupInstr::Backtrack(instr) => {
                    for block in &instr.blocks {
                        reserve_group_block(layout, block);
                    }
                }
            }
        }
        reserve_instr(layout, instr, reserve_group_block);
    }
}

fn reserve_dispatch_block(layout: &mut FrameLayout, block: &pl::DispatchBlock) {
    for instr in block {
        if let pl::InstrKind::Tier(instr) = &instr.node.node {
            match &instr.tier {
                pl::DispatchInstr::Group(instr) => {
                    for exp in &instr.exps_input {
                        reserve_exp(layout, exp);
                    }
                    reserve_group_block(layout, &instr.block);
                }
                pl::DispatchInstr::Route(instr) => {
                    for block in &instr.blocks {
                        reserve_dispatch_block(layout, block);
                    }
                }
            }
        }
        reserve_instr(layout, instr, reserve_dispatch_block);
    }
}

fn reserve_instr<T>(
    layout: &mut FrameLayout,
    instr: &pl::Instr<T>,
    reserve_block: fn(&mut FrameLayout, &pl::Block<T>),
) {
    match &instr.node.node {
        pl::InstrKind::If(instr) => {
            reserve_exp(layout, &instr.exp);
            let _ = instr.iter_exps.clone().prepare(layout);
            reserve_block(layout, &instr.block);
        }
        pl::InstrKind::Hold(instr) => {
            for exp in instr.not_exp.args() {
                reserve_exp(layout, exp);
            }
            let _ = instr.iter_exps.clone().prepare(layout);
            match &instr.hold_case {
                pl::HoldCase::Both(block_l, block_r) => {
                    reserve_block(layout, block_l);
                    reserve_block(layout, block_r);
                }
                pl::HoldCase::Hold(block, _) | pl::HoldCase::NotHold(block, _) => {
                    reserve_block(layout, block);
                }
            }
        }
        pl::InstrKind::Case(instr) => {
            reserve_exp(layout, &instr.exp);
            for case in &instr.cases {
                if let pl::Guard::Cmp(_, _, exp)
                | pl::Guard::Mem(exp)
                | pl::Guard::CheckLetSub(_, _, exp)
                | pl::Guard::CheckLetMatch(_, exp) = &case.guard
                {
                    reserve_exp(layout, exp);
                }
                reserve_block(layout, &case.block);
            }
        }
        pl::InstrKind::Let(instr) => {
            reserve_exp(layout, &instr.exp_l);
            reserve_exp(layout, &instr.exp_r);
            let _ = instr.iter_instrs.clone().prepare(layout);
        }
        pl::InstrKind::Debug(instr) => reserve_exp(layout, &instr.exp),
        pl::InstrKind::Destruct(instr) => {
            for (_, exp) in &instr.bindings {
                reserve_exp(layout, exp);
            }
            reserve_exp(layout, &instr.exp);
        }
        pl::InstrKind::CheckLetSub(instr) => {
            reserve_exp(layout, &instr.exp_l);
            reserve_exp(layout, &instr.exp_r);
            reserve_block(layout, &instr.block);
        }
        pl::InstrKind::CheckLetMatch(instr) => {
            reserve_exp(layout, &instr.exp_l);
            reserve_exp(layout, &instr.exp_r);
            reserve_block(layout, &instr.block);
        }
        pl::InstrKind::OptionGet(instr) => {
            reserve_exp(layout, &instr.exp_l);
            reserve_exp(layout, &instr.exp_r);
            reserve_block(layout, &instr.block);
        }
        pl::InstrKind::Tier(_) => {}
    }
}

pub(crate) fn layout_rel(def: &pl::RelDef) -> FrameLayout {
    let mut layout = FrameLayout::default();
    if let pl::RelDef::Defined(def) = def {
        for exp in &def.exps_input {
            reserve_exp(&mut layout, exp);
        }
        reserve_dispatch_block(&mut layout, &def.block);
        if let Some(block) = &def.block_else_opt {
            reserve_dispatch_block(&mut layout, block);
        }
    }
    layout
}

pub(crate) fn layout_func(def: &pl::MetaFuncDef) -> FrameLayout {
    let mut layout = FrameLayout::default();
    let params = match def {
        pl::MetaFuncDef::Extern(def) => &def.params,
        pl::MetaFuncDef::Builtin(def) => &def.params,
        pl::MetaFuncDef::Table(def) => &def.params,
        pl::MetaFuncDef::Defined(def) => &def.params,
    };
    for param in params {
        reserve_param(&mut layout, param);
    }
    match def {
        pl::MetaFuncDef::Table(def) => {
            for row in &def.rows {
                for exp in &row.exps_input {
                    reserve_exp(&mut layout, exp);
                }
                reserve_exp(&mut layout, &row.exp);
                reserve_group_block(&mut layout, &row.block);
            }
        }
        pl::MetaFuncDef::Defined(def) => {
            reserve_group_block(&mut layout, &def.block);
            if let Some(block) = &def.block_else_opt {
                reserve_group_block(&mut layout, block);
            }
        }
        _ => {}
    }
    layout
}
