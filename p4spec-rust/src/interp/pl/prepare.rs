//! Load-time preparation of PL control flow and expressions
//!
//! Prosification produces annotated blocks with name-based expressions.
//! This traversal borrows that source once to build executable blocks:
//! expressions retain PL syntax and hints while identifiers resolve to slots.
//! Each callable collects its frame layout during the same traversal.

use crate::{
    interp::shared::prepare::Prepare,
    lang::{common::notation::mixfix::Mixfix, pl::ast as pl},
    runtime::envs::interp::{pl::ast_prepared as ast, shared::frame::FrameLayout},
};

// = Slot preparation

// - Notation

fn prepare_not_exp(layout: &mut FrameLayout, mixfix: &pl::NotExp) -> ast::NotExp {
    match mixfix {
        Mixfix::Arg(exp) => Mixfix::Arg(prepare_exp(layout, exp)),
        Mixfix::Atom(atom) => Mixfix::Atom(atom.clone()),
        Mixfix::Brack(atom_l, inner, atom_r) => {
            Mixfix::Brack(atom_l.clone(), Box::new(prepare_not_exp(layout, inner)), atom_r.clone())
        }
        Mixfix::Infix(exp_l, atom, exp_r) => Mixfix::Infix(
            Box::new(prepare_not_exp(layout, exp_l)),
            atom.clone(),
            Box::new(prepare_not_exp(layout, exp_r)),
        ),
        Mixfix::Seq(mixfixes) => Mixfix::Seq(
            mixfixes
                .iter()
                .map(|mixfix| prepare_not_exp(layout, mixfix))
                .collect(),
        ),
    }
}

// - Paths

fn prepare_path(layout: &mut FrameLayout, path: &pl::Path) -> ast::Path {
    let node = match &path.node {
        pl::PathKind::Root => ast::PathKind::Root,
        pl::PathKind::Idx(path, exp) => ast::PathKind::Idx(
            Box::new(prepare_path(layout, path)),
            Box::new(prepare_exp(layout, exp)),
        ),
        pl::PathKind::Slice(path, exp_idx, exp_len) => ast::PathKind::Slice(
            Box::new(prepare_path(layout, path)),
            Box::new(prepare_exp(layout, exp_idx)),
            Box::new(prepare_exp(layout, exp_len)),
        ),
        pl::PathKind::Dot(path, atom) => {
            ast::PathKind::Dot(Box::new(prepare_path(layout, path)), atom.clone())
        }
    };
    crate::note_phrase!(node: node, note: path.note.clone(), span: path.span.clone())
}

// - Arguments

fn prepare_arg(layout: &mut FrameLayout, arg: &pl::Arg) -> ast::Arg {
    let node = match &arg.node {
        pl::ArgKind::Exp(exp) => ast::ArgKind::Exp(Box::new(prepare_exp(layout, exp))),
        pl::ArgKind::Def(id) => ast::ArgKind::Def(id.clone()),
    };
    crate::phrase!(node: node, span: arg.span.clone())
}

// - Expressions

/// Resolves expression slots while retaining PL syntax and prose hints.
fn prepare_exp(layout: &mut FrameLayout, exp: &pl::Exp) -> ast::Exp {
    stacker::maybe_grow(64 * 1024, 1024 * 1024, || {
        use ast::ExpKind as E;
        use pl::ExpKind as P;
        let node = match &exp.node.node {
            P::Bool(value) => E::Bool(*value),
            P::Num(num) => E::Num(num.clone()),
            P::Text(text) => E::Text(text.clone()),
            P::Id(id) => E::Id(id.clone().prepare(layout)),
            P::Un(op, typ, exp) => E::Un(*op, *typ, Box::new(prepare_exp(layout, exp))),
            P::Bin(op, typ, exp_l, exp_r) => E::Bin(
                *op,
                *typ,
                Box::new(prepare_exp(layout, exp_l)),
                Box::new(prepare_exp(layout, exp_r)),
            ),
            P::Cmp(op, typ, exp_l, exp_r) => E::Cmp(
                *op,
                *typ,
                Box::new(prepare_exp(layout, exp_l)),
                Box::new(prepare_exp(layout, exp_r)),
            ),
            P::UpCast(typ, exp) => E::UpCast(typ.clone(), Box::new(prepare_exp(layout, exp))),
            P::DownCast(typ, exp) => E::DownCast(typ.clone(), Box::new(prepare_exp(layout, exp))),
            P::Sub(exp, typ, check) => {
                E::Sub(Box::new(prepare_exp(layout, exp)), typ.clone(), check.clone())
            }
            P::Match(exp, pattern) => E::Match(Box::new(prepare_exp(layout, exp)), pattern.clone()),
            P::Tuple(exps) => E::Tuple(exps.iter().map(|exp| prepare_exp(layout, exp)).collect()),
            P::Case(not_exp) => E::Case(Box::new(prepare_not_exp(layout, not_exp))),
            P::Str(fields) => E::Str(
                fields
                    .iter()
                    .map(|(atom, exp)| (atom.clone(), prepare_exp(layout, exp)))
                    .collect(),
            ),
            P::Opt(exp) => E::Opt(exp.as_ref().map(|exp| Box::new(prepare_exp(layout, exp)))),
            P::List(exps) => E::List(exps.iter().map(|exp| prepare_exp(layout, exp)).collect()),
            P::Cons(exp_head, exp_tail) => E::Cons(
                Box::new(prepare_exp(layout, exp_head)),
                Box::new(prepare_exp(layout, exp_tail)),
            ),
            P::Cat(exp_l, exp_r) => {
                E::Cat(Box::new(prepare_exp(layout, exp_l)), Box::new(prepare_exp(layout, exp_r)))
            }
            P::Mem(exp_elem, exp_list) => E::Mem(
                Box::new(prepare_exp(layout, exp_elem)),
                Box::new(prepare_exp(layout, exp_list)),
            ),
            P::Len(exp) => E::Len(Box::new(prepare_exp(layout, exp))),
            P::Dot(exp, atom) => E::Dot(Box::new(prepare_exp(layout, exp)), atom.clone()),
            P::Idx(exp_base, exp_idx) => E::Idx(
                Box::new(prepare_exp(layout, exp_base)),
                Box::new(prepare_exp(layout, exp_idx)),
            ),
            P::Slice(exp_base, exp_idx, exp_len) => E::Slice(
                Box::new(prepare_exp(layout, exp_base)),
                Box::new(prepare_exp(layout, exp_idx)),
                Box::new(prepare_exp(layout, exp_len)),
            ),
            P::Upd(exp_base, path, exp_new) => E::Upd(
                Box::new(prepare_exp(layout, exp_base)),
                Box::new(prepare_path(layout, path)),
                Box::new(prepare_exp(layout, exp_new)),
            ),
            P::Call(id, targs, args) => E::Call(
                id.clone(),
                targs.clone(),
                args.iter().map(|arg| prepare_arg(layout, arg)).collect(),
            ),
            P::Iter(exp, iter) => {
                E::Iter(Box::new(prepare_exp(layout, exp)), iter.clone().prepare(layout))
            }
        };
        crate::annotated_note_phrase! {
            node: node,
            note: exp.node.note.clone(),
            span: exp.node.span.clone(),
            hints: exp.hints.clone(),
        }
    })
}

fn prepare_exps(layout: &mut FrameLayout, exps: &[pl::Exp]) -> Vec<ast::Exp> {
    exps.iter().map(|exp| prepare_exp(layout, exp)).collect()
}

// - Parameters

/// Prepares parameter patterns, including higher-order signatures.
fn prepare_param(layout: &mut FrameLayout, param: &pl::Param) -> ast::Param {
    let param_kind = match &param.node {
        pl::ParamKind::Exp(typ, exp) => {
            ast::ParamKind::Exp(typ.clone(), Box::new(prepare_exp(layout, exp)))
        }
        pl::ParamKind::Def(id, tparams, params, typ) => ast::ParamKind::Def(
            id.clone(),
            tparams.clone(),
            prepare_params(layout, params),
            typ.clone(),
        ),
    };
    crate::phrase!(node: param_kind, span: param.span.clone())
}

fn prepare_params(layout: &mut FrameLayout, params: &[pl::Param]) -> Vec<ast::Param> {
    params
        .iter()
        .map(|param| prepare_param(layout, param))
        .collect()
}

// - Case analysis

/// Resolves expressions in case guards and checked binding targets.
fn prepare_guard(layout: &mut FrameLayout, guard: &pl::Guard) -> ast::Guard {
    match guard {
        pl::Guard::Bool(cond) => ast::Guard::Bool(*cond),
        pl::Guard::Cmp(op, typ, exp) => ast::Guard::Cmp(*op, *typ, prepare_exp(layout, exp)),
        pl::Guard::Sub(typ, check) => ast::Guard::Sub(typ.clone(), check.clone()),
        pl::Guard::Match(pattern) => ast::Guard::Match(pattern.clone()),
        pl::Guard::Mem(exp) => ast::Guard::Mem(prepare_exp(layout, exp)),
        pl::Guard::CheckLetSub(typ, check, exp) => {
            ast::Guard::CheckLetSub(typ.clone(), check.clone(), prepare_exp(layout, exp))
        }
        pl::Guard::CheckLetMatch(pattern, exp) => {
            ast::Guard::CheckLetMatch(pattern.clone(), prepare_exp(layout, exp))
        }
    }
}

// - Instructions

/// Prepares common instructions while the callback handles the tier.
fn prepare_instr<Tier, PreparedTier>(
    layout: &mut FrameLayout,
    instr: &pl::Instr<Tier>,
    prepare_tier: fn(&mut FrameLayout, &Tier) -> PreparedTier,
) -> ast::Instr<PreparedTier> {
    // Protect recursive blocks independently of callable depth
    stacker::maybe_grow(64 * 1024, 1024 * 1024, || {
        let instr_kind = match &instr.node.node {
            pl::InstrKind::If(instr) => ast::InstrKind::If(ast::IfInstr {
                exp: prepare_exp(layout, &instr.exp),
                iter_exps: instr.iter_exps.clone().prepare(layout),
                block: prepare_block(layout, &instr.block, prepare_tier),
                dangle: instr.dangle,
            }),
            pl::InstrKind::Hold(instr) => ast::InstrKind::Hold(ast::HoldInstr {
                id: instr.id.clone(),
                not_exp: prepare_not_exp(layout, &instr.not_exp),
                iter_exps: instr.iter_exps.clone().prepare(layout),
                hold_case: match &instr.hold_case {
                    pl::HoldCase::Both(block_l, block_r) => ast::HoldCase::Both(
                        prepare_block(layout, block_l, prepare_tier),
                        prepare_block(layout, block_r, prepare_tier),
                    ),
                    pl::HoldCase::Hold(block, dangle) => {
                        ast::HoldCase::Hold(prepare_block(layout, block, prepare_tier), *dangle)
                    }
                    pl::HoldCase::NotHold(block, dangle) => {
                        ast::HoldCase::NotHold(prepare_block(layout, block, prepare_tier), *dangle)
                    }
                },
            }),
            pl::InstrKind::Case(instr) => ast::InstrKind::Case(ast::CaseInstr {
                exp: prepare_exp(layout, &instr.exp),
                cases: instr
                    .cases
                    .iter()
                    .map(|case| ast::Case {
                        guard: prepare_guard(layout, &case.guard),
                        block: prepare_block(layout, &case.block, prepare_tier),
                    })
                    .collect(),
                dangle: instr.dangle,
            }),
            pl::InstrKind::Let(instr) => ast::InstrKind::Let(ast::LetInstr {
                exp_l: prepare_exp(layout, &instr.exp_l),
                exp_r: prepare_exp(layout, &instr.exp_r),
                iter_instrs: instr.iter_instrs.clone().prepare(layout),
            }),
            pl::InstrKind::Debug(instr) => {
                ast::InstrKind::Debug(ast::DebugInstr { exp: prepare_exp(layout, &instr.exp) })
            }
            pl::InstrKind::Destruct(instr) => ast::InstrKind::Destruct(ast::DestructInstr {
                bindings: instr
                    .bindings
                    .iter()
                    .map(|(name, exp)| (name.clone(), prepare_exp(layout, exp)))
                    .collect(),
                exp: prepare_exp(layout, &instr.exp),
            }),
            pl::InstrKind::CheckLetSub(instr) => {
                ast::InstrKind::CheckLetSub(ast::CheckLetSubInstr {
                    typ: instr.typ.clone(),
                    subcheck: instr.subcheck.clone(),
                    exp_l: prepare_exp(layout, &instr.exp_l),
                    exp_r: prepare_exp(layout, &instr.exp_r),
                    block: prepare_block(layout, &instr.block, prepare_tier),
                })
            }
            pl::InstrKind::CheckLetMatch(instr) => {
                ast::InstrKind::CheckLetMatch(ast::CheckLetMatchInstr {
                    pattern: instr.pattern.clone(),
                    exp_l: prepare_exp(layout, &instr.exp_l),
                    exp_r: prepare_exp(layout, &instr.exp_r),
                    block: prepare_block(layout, &instr.block, prepare_tier),
                })
            }
            pl::InstrKind::OptionGet(instr) => ast::InstrKind::OptionGet(ast::OptionGetInstr {
                exp_l: prepare_exp(layout, &instr.exp_l),
                exp_r: prepare_exp(layout, &instr.exp_r),
                block: prepare_block(layout, &instr.block, prepare_tier),
            }),
            pl::InstrKind::Tier(instr) => {
                ast::InstrKind::Tier(ast::TierInstr { tier: prepare_tier(layout, &instr.tier) })
            }
        };
        // Fallthrough notes remain descriptive metadata for prose diagnostics
        crate::annotated_note_phrase! {
            node: instr_kind, note: instr.node.note.clone(),
            span: instr.node.span.clone(), hints: instr.hints.clone(),
        }
    })
}

// - Blocks

fn prepare_block<Tier, PreparedTier>(
    layout: &mut FrameLayout,
    block: &pl::Block<Tier>,
    prepare_tier: fn(&mut FrameLayout, &Tier) -> PreparedTier,
) -> ast::Block<PreparedTier> {
    block
        .iter()
        .map(|instr| prepare_instr(layout, instr, prepare_tier))
        .collect()
}

// - Group-body tier

/// Prepares group conclusions, bindings, and backtracking alternatives.
fn prepare_group(layout: &mut FrameLayout, instr: &pl::GroupInstr) -> ast::GroupInstr {
    match instr {
        pl::GroupInstr::Result(instr) => ast::GroupInstr::Result(ast::ResultInstr {
            rel_signature: instr.rel_signature.clone(),
            exps_output: prepare_exps(layout, &instr.exps_output),
        }),
        pl::GroupInstr::Return(instr) => {
            ast::GroupInstr::Return(ast::ReturnInstr { exp: prepare_exp(layout, &instr.exp) })
        }
        pl::GroupInstr::Rule(instr) => ast::GroupInstr::Rule(ast::RuleInstr {
            id: instr.id.clone(),
            not_exp: prepare_not_exp(layout, &instr.not_exp),
            input_hint: instr.input_hint.clone(),
            iter_instrs: instr.iter_instrs.clone().prepare(layout),
        }),
        pl::GroupInstr::Backtrack(instr) => ast::GroupInstr::Backtrack(ast::BacktrackInstr {
            blocks: instr
                .blocks
                .iter()
                .map(|block| prepare_block(layout, block, prepare_group))
                .collect(),
        }),
    }
}

// - Dispatch tier

/// Prepares relation routing and the group bodies it selects.
fn prepare_dispatch(layout: &mut FrameLayout, instr: &pl::DispatchInstr) -> ast::DispatchInstr {
    match instr {
        pl::DispatchInstr::Group(instr) => ast::DispatchInstr::Group(ast::RuleGroupInstr {
            id_rel: instr.id_rel.clone(),
            id_group: instr.id_group.clone(),
            rel_signature: instr.rel_signature.clone(),
            exps_input: prepare_exps(layout, &instr.exps_input),
            block: prepare_block(layout, &instr.block, prepare_group),
        }),
        pl::DispatchInstr::Route(instr) => ast::DispatchInstr::Route(ast::RouteInstr {
            blocks: instr
                .blocks
                .iter()
                .map(|block| prepare_block(layout, block, prepare_dispatch))
                .collect(),
        }),
    }
}

// = Relation definitions

impl Prepare for &pl::RelDef {
    type Output = ast::RelDef;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        match self {
            pl::RelDef::Extern(rel) => ast::RelDef::Extern(ast::ExternRel {
                id: rel.id.clone(),
                rel_signature: rel.rel_signature.clone(),
                exps_input: prepare_exps(layout, &rel.exps_input),
            }),
            pl::RelDef::Defined(rel) => ast::RelDef::Defined(ast::DefinedRel {
                id: rel.id.clone(),
                rel_signature: rel.rel_signature.clone(),
                exps_input: prepare_exps(layout, &rel.exps_input),
                block: prepare_block(layout, &rel.block, prepare_dispatch),
                block_else_opt: rel
                    .block_else_opt
                    .as_ref()
                    .map(|block| prepare_block(layout, block, prepare_dispatch)),
            }),
        }
    }
}

// = Meta-function definitions

impl Prepare for &pl::MetaFuncDef {
    type Output = ast::MetaFuncDef;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        match self {
            pl::MetaFuncDef::Extern(func) => ast::MetaFuncDef::Extern(ast::ExternFunc {
                id: func.id.clone(),
                tparams: func.tparams.clone(),
                params: prepare_params(layout, &func.params),
                typ: func.typ.clone(),
            }),
            pl::MetaFuncDef::Builtin(func) => ast::MetaFuncDef::Builtin(ast::BuiltinFunc {
                id: func.id.clone(),
                tparams: func.tparams.clone(),
                params: prepare_params(layout, &func.params),
                typ: func.typ.clone(),
            }),
            pl::MetaFuncDef::Table(func) => ast::MetaFuncDef::Table(ast::TableFunc {
                id: func.id.clone(),
                params: prepare_params(layout, &func.params),
                typ: func.typ.clone(),
                rows: func
                    .rows
                    .iter()
                    .map(|row| ast::TableRow {
                        exps_input: prepare_exps(layout, &row.exps_input),
                        exp: prepare_exp(layout, &row.exp),
                        block: prepare_block(layout, &row.block, prepare_group),
                    })
                    .collect(),
            }),
            pl::MetaFuncDef::Defined(func) => ast::MetaFuncDef::Defined(ast::DefinedFunc {
                id: func.id.clone(),
                tparams: func.tparams.clone(),
                params: prepare_params(layout, &func.params),
                typ: func.typ.clone(),
                block: prepare_block(layout, &func.block, prepare_group),
                block_else_opt: func
                    .block_else_opt
                    .as_ref()
                    .map(|block| prepare_block(layout, block, prepare_group)),
            }),
        }
    }
}
