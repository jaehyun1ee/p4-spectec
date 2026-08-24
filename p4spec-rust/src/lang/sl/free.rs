use crate::lang::il;

use super::ast::*;

// Identifier set

pub type FreeVars = il::free::FreeVars;

pub fn empty() -> FreeVars {
    il::free::empty()
}

pub fn singleton(id: &Id) -> FreeVars {
    il::free::singleton(id)
}

fn add(set_a: FreeVars, set_b: FreeVars) -> FreeVars {
    set_a.into_iter().chain(set_b).collect()
}

fn many<Item>(items: &[Item], collect: impl Fn(&Item) -> FreeVars) -> FreeVars {
    items
        .iter()
        .fold(empty(), |set, item| add(set, collect(item)))
}

// Collect free identifiers

// Expressions

pub fn free_exp(exp: &Exp) -> FreeVars {
    il::free::free_exp(exp)
}

pub fn free_exps(exps: &[Exp]) -> FreeVars {
    il::free::free_exps(exps)
}

// Paths

pub fn free_path(path: &Path) -> FreeVars {
    il::free::free_path(path)
}

// Parameters

pub fn free_param(param: &Param) -> FreeVars {
    match &param.node {
        ParamKind::ExpP(_, exp) => free_exp(exp),
        ParamKind::DefP(..) => empty(),
    }
}

pub fn free_params(params: &[Param]) -> FreeVars {
    many(params, free_param)
}

// Arguments

pub fn free_arg(arg: &Arg) -> FreeVars {
    il::free::free_arg(arg)
}

pub fn free_args(args: &[Arg]) -> FreeVars {
    il::free::free_args(args)
}

pub fn free_cases(cases: &[Case]) -> FreeVars {
    many(cases, |(guard, block)| {
        add(free_guard(guard), free_block(block))
    })
}

pub fn free_guard(guard: &Guard) -> FreeVars {
    match guard {
        Guard::BoolG(_) | Guard::SubG(..) | Guard::MatchG(_) => empty(),
        Guard::CmpG(_, _, exp) | Guard::MemG(exp) => free_exp(exp),
    }
}

pub fn free_instr(instr: &Instr) -> FreeVars {
    match &instr.kind {
        InstrKind::IfI(exp, _, block, _) => add(free_exp(exp), free_block(block)),
        InstrKind::HoldI(_, notexp, _, _) => many(&notexp.args(), |exp| free_exp(exp)),
        InstrKind::CaseI(exp, cases, _) => add(free_exp(exp), free_cases(cases)),
        InstrKind::GroupI(_, _, exps, block) => add(free_exps(exps), free_block(block)),
        InstrKind::LetI(exp_l, exp_r, _, block) => {
            add(free_exp(exp_l), add(free_exp(exp_r), free_block(block)))
        }
        InstrKind::RuleI(_, notexp, _, _, block) => {
            add(many(&notexp.args(), |exp| free_exp(exp)), free_block(block))
        }
        InstrKind::ResultI(_, exps) => free_exps(exps),
        InstrKind::ReturnI(exp) => free_exp(exp),
        InstrKind::DebugI(exp, instr) => add(free_exp(exp), free_instr(instr)),
    }
}

pub fn free_instrs(instrs: &[Instr]) -> FreeVars {
    many(instrs, free_instr)
}

pub fn free_block(block: &Block) -> FreeVars {
    free_instrs(block)
}
