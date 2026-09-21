//! Lift nested calls into explicit SL let instructions

mod lift;

use crate::lang::{common::ds::set::IdSet, hints::input, sl::ast as sl, traits::free::FreeIds};

use self::lift::{
    LiftedCall, RootCallPolicy, ids_bound_by_exp_iters, ids_bound_by_instr_iters, lift_first_exp,
    lift_first_exps,
};
use super::{ProseError, ProseErrorKind};

// == Instruction lifting

fn lift_instr_call(
    instr_sl: &mut sl::Instr,
    ids_used: &mut IdSet,
) -> Result<Option<LiftedCall>, ProseError> {
    let span = instr_sl.span.clone();
    Ok(match &mut instr_sl.node {
        sl::InstrKind::Let(instr_sl) => lift_first_exp(
            &mut instr_sl.exp_r,
            RootCallPolicy::Preserve,
            &ids_bound_by_instr_iters(&instr_sl.iter_instrs),
            ids_used,
        ),
        sl::InstrKind::Rule(instr_sl) => {
            let exps_sl = instr_sl
                .not_exp
                .args()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let (mut exps_input_sl, exps_output_sl) =
                input::split(&instr_sl.input_hint, exps_sl)
                    .map_err(|error| ProseError::new(ProseErrorKind::Input(error), span.clone()))?;
            let lifted_call = lift_first_exps(
                &mut exps_input_sl,
                RootCallPolicy::Lift,
                &ids_bound_by_instr_iters(&instr_sl.iter_instrs),
                ids_used,
            );
            if lifted_call.is_some() {
                let exps_sl =
                    input::combine(&instr_sl.input_hint, exps_input_sl, exps_output_sl)
                        .map_err(|error| ProseError::new(ProseErrorKind::Input(error), span))?;
                let mut exps_sl = exps_sl.into_iter();
                instr_sl.not_exp = instr_sl.not_exp.map(|_| exps_sl.next().unwrap());
            }
            lifted_call
        }
        sl::InstrKind::Hold(instr_sl) => {
            let mut exps_sl = instr_sl
                .not_exp
                .args()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let lifted_call = lift_first_exps(
                &mut exps_sl,
                RootCallPolicy::Lift,
                &ids_bound_by_exp_iters(&instr_sl.iter_exps),
                ids_used,
            );
            if lifted_call.is_some() {
                let mut exps_sl = exps_sl.into_iter();
                instr_sl.not_exp = instr_sl.not_exp.map(|_| exps_sl.next().unwrap());
            }
            lifted_call
        }
        sl::InstrKind::Result(instr_sl) => {
            lift_first_exps(&mut instr_sl.exps, RootCallPolicy::Preserve, &IdSet::new(), ids_used)
        }
        sl::InstrKind::Return(instr_sl) => {
            lift_first_exp(&mut instr_sl.exp, RootCallPolicy::Preserve, &IdSet::new(), ids_used)
        }
        sl::InstrKind::If(_)
        | sl::InstrKind::Case(_)
        | sl::InstrKind::Group(_)
        | sl::InstrKind::Debug(_) => None,
    })
}

// == Nested blocks

fn expand_hold_case(
    ids_used: &mut IdSet,
    hold_case_sl: sl::HoldCase,
) -> Result<sl::HoldCase, ProseError> {
    Ok(match hold_case_sl {
        sl::HoldCase::Both(block_hold_sl, block_not_hold_sl) => sl::HoldCase::Both(
            expand_block(ids_used, block_hold_sl)?,
            expand_block(ids_used, block_not_hold_sl)?,
        ),
        sl::HoldCase::Hold(block_sl, dangle) => {
            sl::HoldCase::Hold(expand_block(ids_used, block_sl)?, dangle)
        }
        sl::HoldCase::NotHold(block_sl, dangle) => {
            sl::HoldCase::NotHold(expand_block(ids_used, block_sl)?, dangle)
        }
    })
}

fn expand_subblocks(ids_used: &mut IdSet, instr_sl: sl::Instr) -> Result<sl::Instr, ProseError> {
    let span = instr_sl.span;
    let instr_kind_sl = match instr_sl.node {
        sl::InstrKind::Let(mut instr_sl) => {
            instr_sl.block = expand_block(ids_used, instr_sl.block)?;
            sl::InstrKind::Let(instr_sl)
        }
        sl::InstrKind::Rule(mut instr_sl) => {
            instr_sl.block = expand_block(ids_used, instr_sl.block)?;
            sl::InstrKind::Rule(instr_sl)
        }
        sl::InstrKind::If(mut instr_sl) => {
            instr_sl.block = expand_block(ids_used, instr_sl.block)?;
            sl::InstrKind::If(instr_sl)
        }
        sl::InstrKind::Hold(mut instr_sl) => {
            instr_sl.hold_case = expand_hold_case(ids_used, instr_sl.hold_case)?;
            sl::InstrKind::Hold(instr_sl)
        }
        sl::InstrKind::Case(mut instr_sl) => {
            for case_sl in &mut instr_sl.cases {
                case_sl.block = expand_block(ids_used, std::mem::take(&mut case_sl.block))?;
            }
            sl::InstrKind::Case(instr_sl)
        }
        sl::InstrKind::Group(mut instr_sl) => {
            instr_sl.block = expand_block(ids_used, instr_sl.block)?;
            sl::InstrKind::Group(instr_sl)
        }
        sl::InstrKind::Debug(mut instr_sl) => {
            instr_sl.instr = Box::new(expand_instr(ids_used, *instr_sl.instr)?);
            sl::InstrKind::Debug(instr_sl)
        }
        sl::InstrKind::Result(instr_sl) => sl::InstrKind::Result(instr_sl),
        sl::InstrKind::Return(instr_sl) => sl::InstrKind::Return(instr_sl),
    };
    Ok(crate::phrase! { node: instr_kind_sl, span: span })
}

fn expand_instr(ids_used: &mut IdSet, instr_sl: sl::Instr) -> Result<sl::Instr, ProseError> {
    let mut instr_sl = expand_subblocks(ids_used, instr_sl)?;
    let mut calls_lifted = Vec::new();
    while let Some(call_lifted) = lift_instr_call(&mut instr_sl, ids_used)? {
        calls_lifted.push(call_lifted);
    }
    if calls_lifted.is_empty() {
        return Ok(instr_sl);
    }
    for call_lifted in calls_lifted.into_iter().rev() {
        instr_sl = call_lifted.wrap_instr(instr_sl);
    }
    expand_instr(ids_used, instr_sl)
}

fn expand_block(ids_used: &mut IdSet, block_sl: sl::Block) -> Result<sl::Block, ProseError> {
    block_sl
        .into_iter()
        .map(|instr_sl| expand_instr(ids_used, instr_sl))
        .collect()
}

// == Definitions

fn expand_def(mut def_sl: sl::Def) -> Result<sl::Def, ProseError> {
    match &mut def_sl.node {
        sl::DefKind::Rel(sl::RelDef::Defined(def_rel_sl)) => {
            let mut ids_used = def_rel_sl
                .exps_input
                .free_ids()
                .union(def_rel_sl.block.free_ids());
            if let Some(block_else_sl) = &def_rel_sl.block_else {
                ids_used.append(block_else_sl.free_ids());
            }
            let mut ids_body = ids_used.clone();
            def_rel_sl.block = expand_block(&mut ids_body, std::mem::take(&mut def_rel_sl.block))?;
            if let Some(block_else_sl) = def_rel_sl.block_else.take() {
                def_rel_sl.block_else = Some(expand_block(&mut ids_used, block_else_sl)?);
            }
        }
        sl::DefKind::MetaFunc(sl::MetaFuncDef::Table(def_func_sl)) => {
            for row_sl in &mut def_func_sl.table_rows {
                let mut ids_used = row_sl
                    .exps_input
                    .free_ids()
                    .union(row_sl.exp.free_ids())
                    .union(row_sl.block.free_ids());
                row_sl.block = expand_block(&mut ids_used, std::mem::take(&mut row_sl.block))?;
            }
        }
        sl::DefKind::MetaFunc(sl::MetaFuncDef::Defined(def_func_sl)) => {
            let mut ids_used = def_func_sl
                .params
                .free_ids()
                .union(def_func_sl.block.free_ids());
            if let Some(block_else_sl) = &def_func_sl.block_else {
                ids_used.append(block_else_sl.free_ids());
            }
            let mut ids_body = ids_used.clone();
            def_func_sl.block =
                expand_block(&mut ids_body, std::mem::take(&mut def_func_sl.block))?;
            if let Some(block_else_sl) = def_func_sl.block_else.take() {
                def_func_sl.block_else = Some(expand_block(&mut ids_used, block_else_sl)?);
            }
        }
        _ => {}
    }
    Ok(def_sl)
}

// == Entry point

pub(super) fn expand_spec(spec_sl: sl::Spec) -> Result<sl::Spec, ProseError> {
    spec_sl.into_iter().map(expand_def).collect()
}
