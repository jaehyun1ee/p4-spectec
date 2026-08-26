//! Free identifiers in structured-language data

use crate::{
    domain::sets::{self, IdSet},
    lang::il,
};

use super::ast::*;

// == Free identifiers

fn frees<T>(items: &[T], free: impl Fn(&T) -> IdSet) -> IdSet {
    sets::unions(items.iter().map(free))
}

// - Expressions

/// Collects free identifiers from exp
pub fn free_exp(exp: &Exp) -> IdSet {
    il::free::free_exp(exp)
}

/// Collects free identifiers from exps
pub fn free_exps(exps: &[Exp]) -> IdSet {
    il::free::free_exps(exps)
}

// - Paths

/// Collects free identifiers from path
pub fn free_path(path: &Path) -> IdSet {
    il::free::free_path(path)
}

// - Parameters

/// Collects free identifiers from param
pub fn free_param(param: &Param) -> IdSet {
    match &param.node {
        ParamKind::Exp(_, exp) => free_exp(exp),
        ParamKind::Def(..) => sets::empty(),
    }
}

/// Collects free identifiers from params
pub fn free_params(params: &[Param]) -> IdSet {
    frees(params, free_param)
}

// - Arguments

/// Collects free identifiers from arg
pub fn free_arg(arg: &Arg) -> IdSet {
    il::free::free_arg(arg)
}

/// Collects free identifiers from args
pub fn free_args(args: &[Arg]) -> IdSet {
    il::free::free_args(args)
}

// - Cases

/// Collects free identifiers from cases
pub fn free_cases(cases: &[Case]) -> IdSet {
    frees(cases, |case| {
        sets::union(free_guard(&case.guard), free_block(&case.block))
    })
}

// - Guards

/// Collects free identifiers from guard
pub fn free_guard(guard: &Guard) -> IdSet {
    match guard {
        Guard::Bool(_) | Guard::Sub(..) | Guard::Match(_) => sets::empty(),
        Guard::Cmp(_, _, exp) | Guard::Mem(exp) => free_exp(exp),
    }
}

// - Instructions

/// Collects free identifiers from instr
pub fn free_instr(instr: &Instr) -> IdSet {
    match &instr.node.kind {
        InstrKind::If(IfInstr { exp, block, .. }) => sets::union(free_exp(exp), free_block(block)),
        InstrKind::Hold(HoldInstr { not_exp, .. }) => frees(&not_exp.args(), |exp| free_exp(exp)),
        InstrKind::Case(CaseInstr { exp, cases, .. }) => {
            sets::union(free_exp(exp), free_cases(cases))
        }
        InstrKind::Group(GroupInstr { exps, block, .. }) => {
            sets::union(free_exps(exps), free_block(block))
        }
        InstrKind::Let(LetInstr {
            exp_l,
            exp_r,
            block,
            ..
        }) => sets::union(
            free_exp(exp_l),
            sets::union(free_exp(exp_r), free_block(block)),
        ),
        InstrKind::Rule(RuleInstr { not_exp, block, .. }) => sets::union(
            frees(&not_exp.args(), |exp| free_exp(exp)),
            free_block(block),
        ),
        InstrKind::Result(ResultInstr { exps, .. }) => free_exps(exps),
        InstrKind::Return(ReturnInstr { exp }) => free_exp(exp),
        InstrKind::Debug(DebugInstr { exp, instr }) => {
            sets::union(free_exp(exp), free_instr(instr))
        }
    }
}

/// Collects free identifiers from instrs
pub fn free_instrs(instrs: &[Instr]) -> IdSet {
    frees(instrs, free_instr)
}

/// Collects free identifiers from block
pub fn free_block(block: &Block) -> IdSet {
    free_instrs(block)
}
