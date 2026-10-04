//! Registers execution metadata after SL global storage is complete
//!
//! The visitor follows evaluated expressions in stored instruction blocks.

use crate::interp::shared::prepare::plans::EvalPlans;
use crate::runtime::envs::interp::sl::ast_prepared as ast;

/// Visits evaluated expressions and nested instruction blocks.
fn block(instrs: &[ast::Instr], plans: &mut EvalPlans) {
    for instr in instrs {
        stacker::maybe_grow(64 * 1024, 1024 * 1024, || match &instr.node {
            ast::InstrKind::If(instr) => {
                plans.register_condition(&instr.exp);
                plans.constructs.register(&instr.exp);
                block(&instr.block, plans);
            }
            ast::InstrKind::Hold(instr) => {
                for exp in instr.not_exp.args() {
                    plans.constructs.register(exp);
                }
                match &instr.hold_case {
                    ast::HoldCase::Both(block_hold, block_not) => {
                        block(block_hold, plans);
                        block(block_not, plans);
                    }
                    ast::HoldCase::Hold(block_inner, _)
                    | ast::HoldCase::NotHold(block_inner, _) => {
                        block(block_inner, plans);
                    }
                }
            }
            ast::InstrKind::Case(instr) => {
                plans.register_condition(&instr.exp);
                plans.constructs.register(&instr.exp);
                for case in &instr.cases {
                    if let ast::Guard::Cmp(_, _, exp) | ast::Guard::Mem(exp) = &case.guard {
                        plans.constructs.register(exp);
                    }
                    block(&case.block, plans);
                }
            }
            ast::InstrKind::Group(instr) => block(&instr.block, plans),
            ast::InstrKind::Let(instr) => {
                plans.constructs.register(&instr.exp_r);
                block(&instr.block, plans);
            }
            ast::InstrKind::Rule(instr) => {
                for exp in instr.not_exp.args() {
                    plans.constructs.register(exp);
                }
                block(&instr.block, plans);
            }
            ast::InstrKind::Result(instr) => {
                for exp in &instr.exps {
                    plans.constructs.register(exp);
                }
            }
            ast::InstrKind::Return(instr) => plans.constructs.register(&instr.exp),
            ast::InstrKind::Debug(instr) => {
                plans.constructs.register(&instr.exp);
                block(std::slice::from_ref(&instr.instr), plans);
            }
        });
    }
}

/// Registers evaluated expressions and conditions in a stored relation body.
pub(super) fn rel(rel: &ast::RelDef, plans: &mut EvalPlans) {
    if let ast::RelDef::Defined(rel) = rel {
        block(&rel.block, plans);
        if let Some(block_else) = &rel.block_else {
            block(block_else, plans);
        }
    }
}

/// Registers evaluated expressions and conditions in function and table bodies.
pub(super) fn func(func: &ast::MetaFuncDef, plans: &mut EvalPlans) {
    match func {
        ast::MetaFuncDef::Table(func) => {
            for row in &func.table_rows {
                block(&row.block, plans);
            }
        }
        ast::MetaFuncDef::Defined(func) => {
            block(&func.block, plans);
            if let Some(block_else) = &func.block_else {
                block(block_else, plans);
            }
        }
        ast::MetaFuncDef::Extern(_) | ast::MetaFuncDef::Builtin(_) => {}
    }
}
