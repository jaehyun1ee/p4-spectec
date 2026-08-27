//! Syntax equality for prose-language data
//!
//! Ignores source regions, inferred notes, and prose annotations

use crate::lang::traits::eq::SyntaxEq;

use super::ast::*;

// == Syntax equality

// - Expressions

impl SyntaxEq for ExpNode {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.node.kind.syntax_eq(&other.node.kind)
    }
}

impl SyntaxEq for Exp {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.node.syntax_eq(&other.node)
    }
}

impl SyntaxEq for ExpKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ExpKind::Bool(value_l), ExpKind::Bool(value_r)) => value_l == value_r,
            (ExpKind::Num(value_l), ExpKind::Num(value_r)) => value_l == value_r,
            (ExpKind::Text(value_l), ExpKind::Text(value_r)) => value_l == value_r,
            (ExpKind::Var(id_l), ExpKind::Var(id_r)) => id_l.syntax_eq(id_r),
            (ExpKind::Un(op_l, typ_l, exp_l), ExpKind::Un(op_r, typ_r, exp_r)) => {
                op_l.syntax_eq(op_r) && typ_l.syntax_eq(typ_r) && exp_l.syntax_eq(exp_r)
            }
            (
                ExpKind::Bin(op_l, typ_l, exp_l_l, exp_r_l),
                ExpKind::Bin(op_r, typ_r, exp_l_r, exp_r_r),
            ) => {
                op_l.syntax_eq(op_r)
                    && typ_l.syntax_eq(typ_r)
                    && exp_l_l.syntax_eq(exp_l_r)
                    && exp_r_l.syntax_eq(exp_r_r)
            }
            (
                ExpKind::Cmp(op_l, typ_l, exp_l_l, exp_r_l),
                ExpKind::Cmp(op_r, typ_r, exp_l_r, exp_r_r),
            ) => {
                op_l.syntax_eq(op_r)
                    && typ_l.syntax_eq(typ_r)
                    && exp_l_l.syntax_eq(exp_l_r)
                    && exp_r_l.syntax_eq(exp_r_r)
            }
            (ExpKind::UpCast(typ_l, exp_l), ExpKind::UpCast(typ_r, exp_r))
            | (ExpKind::DownCast(typ_l, exp_l), ExpKind::DownCast(typ_r, exp_r)) => {
                typ_l.syntax_eq(typ_r) && exp_l.syntax_eq(exp_r)
            }
            (ExpKind::Sub(exp_l, typ_l, _), ExpKind::Sub(exp_r, typ_r, _)) => {
                exp_l.syntax_eq(exp_r) && typ_l.syntax_eq(typ_r)
            }
            (ExpKind::Match(exp_l, pattern_l), ExpKind::Match(exp_r, pattern_r)) => {
                exp_l.syntax_eq(exp_r) && pattern_l.syntax_eq(pattern_r)
            }
            (ExpKind::Tuple(exps_l), ExpKind::Tuple(exps_r))
            | (ExpKind::List(exps_l), ExpKind::List(exps_r)) => exps_l.syntax_eq(exps_r),
            (ExpKind::Case(not_exp_l), ExpKind::Case(not_exp_r)) => not_exp_l.syntax_eq(not_exp_r),
            (ExpKind::Str(fields_l), ExpKind::Str(fields_r)) => {
                fields_l.len() == fields_r.len()
                    && fields_l
                        .iter()
                        .zip(fields_r)
                        .all(|((atom_l, exp_l), (atom_r, exp_r))| {
                            atom_l.syntax_eq(atom_r) && exp_l.syntax_eq(exp_r)
                        })
            }
            (ExpKind::Opt(Some(exp_l)), ExpKind::Opt(Some(exp_r))) => exp_l.syntax_eq(exp_r),
            (ExpKind::Opt(None), ExpKind::Opt(None)) => true,
            (ExpKind::Cons(exp_l_l, exp_r_l), ExpKind::Cons(exp_l_r, exp_r_r))
            | (ExpKind::Cat(exp_l_l, exp_r_l), ExpKind::Cat(exp_l_r, exp_r_r))
            | (ExpKind::Mem(exp_l_l, exp_r_l), ExpKind::Mem(exp_l_r, exp_r_r)) => {
                exp_l_l.syntax_eq(exp_l_r) && exp_r_l.syntax_eq(exp_r_r)
            }
            (ExpKind::Len(exp_l), ExpKind::Len(exp_r)) => exp_l.syntax_eq(exp_r),
            (ExpKind::Dot(exp_l, atom_l), ExpKind::Dot(exp_r, atom_r)) => {
                exp_l.syntax_eq(exp_r) && atom_l.syntax_eq(atom_r)
            }
            (ExpKind::Idx(exp_b_l, exp_i_l), ExpKind::Idx(exp_b_r, exp_i_r)) => {
                exp_b_l.syntax_eq(exp_b_r) && exp_i_l.syntax_eq(exp_i_r)
            }
            (
                ExpKind::Slice(exp_b_l, exp_i_l, exp_n_l),
                ExpKind::Slice(exp_b_r, exp_i_r, exp_n_r),
            ) => {
                exp_b_l.syntax_eq(exp_b_r)
                    && exp_i_l.syntax_eq(exp_i_r)
                    && exp_n_l.syntax_eq(exp_n_r)
            }
            (ExpKind::Upd(exp_b_l, path_l, exp_f_l), ExpKind::Upd(exp_b_r, path_r, exp_f_r)) => {
                exp_b_l.syntax_eq(exp_b_r) && path_l.syntax_eq(path_r) && exp_f_l.syntax_eq(exp_f_r)
            }
            (ExpKind::Call(id_l, targs_l, args_l), ExpKind::Call(id_r, targs_r, args_r)) => {
                id_l.syntax_eq(id_r) && targs_l.syntax_eq(targs_r) && args_l.syntax_eq(args_r)
            }
            (ExpKind::Iter(exp_l, iter_exp_l), ExpKind::Iter(exp_r, iter_exp_r)) => {
                exp_l.syntax_eq(exp_r) && iter_exp_l.syntax_eq(iter_exp_r)
            }
            _ => false,
        }
    }
}

impl SyntaxEq for [Exp] {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .zip(other)
                .all(|(exp_l, exp_r)| exp_l.syntax_eq(exp_r))
    }
}

// - Paths and arguments

impl SyntaxEq for Path {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.node.kind.syntax_eq(&other.node.kind)
    }
}

impl SyntaxEq for PathKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PathKind::Root, PathKind::Root) => true,
            (PathKind::Idx(path_l, exp_l), PathKind::Idx(path_r, exp_r)) => {
                path_l.syntax_eq(path_r) && exp_l.syntax_eq(exp_r)
            }
            (
                PathKind::Slice(path_l, exp_l_l, exp_r_l),
                PathKind::Slice(path_r, exp_l_r, exp_r_r),
            ) => {
                path_l.syntax_eq(path_r) && exp_l_l.syntax_eq(exp_l_r) && exp_r_l.syntax_eq(exp_r_r)
            }
            (PathKind::Dot(path_l, atom_l), PathKind::Dot(path_r, atom_r)) => {
                path_l.syntax_eq(path_r) && atom_l.syntax_eq(atom_r)
            }
            _ => false,
        }
    }
}

impl SyntaxEq for Param {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.node.syntax_eq(&other.node)
    }
}

impl SyntaxEq for ParamKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ParamKind::Exp(typ_l, exp_l), ParamKind::Exp(typ_r, exp_r)) => {
                typ_l.syntax_eq(typ_r) && exp_l.syntax_eq(exp_r)
            }
            (
                ParamKind::Def(id_l, tparams_l, params_l, typ_l),
                ParamKind::Def(id_r, tparams_r, params_r, typ_r),
            ) => {
                id_l.syntax_eq(id_r)
                    && tparams_l.syntax_eq(tparams_r)
                    && params_l.syntax_eq(params_r)
                    && typ_l.syntax_eq(typ_r)
            }
            _ => false,
        }
    }
}

impl SyntaxEq for [Param] {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .zip(other)
                .all(|(param_l, param_r)| param_l.syntax_eq(param_r))
    }
}

impl SyntaxEq for Arg {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.node.syntax_eq(&other.node)
    }
}

impl SyntaxEq for ArgKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ArgKind::Exp(exp_l), ArgKind::Exp(exp_r)) => exp_l.syntax_eq(exp_r),
            (ArgKind::Def(id_l), ArgKind::Def(id_r)) => id_l.syntax_eq(id_r),
            _ => false,
        }
    }
}

impl SyntaxEq for [Arg] {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .zip(other)
                .all(|(arg_l, arg_r)| arg_l.syntax_eq(arg_r))
    }
}

// - Control flow

impl<Tier> SyntaxEq for HoldCase<Tier>
where
    Tier: SyntaxEq,
{
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                HoldCase::Both(block_hold_l, block_not_hold_l),
                HoldCase::Both(block_hold_r, block_not_hold_r),
            ) => {
                block_hold_l.syntax_eq(block_hold_r) && block_not_hold_l.syntax_eq(block_not_hold_r)
            }
            (HoldCase::Hold(block_l, dangle_l), HoldCase::Hold(block_r, dangle_r))
            | (HoldCase::NotHold(block_l, dangle_l), HoldCase::NotHold(block_r, dangle_r)) => {
                block_l.syntax_eq(block_r) && dangle_l == dangle_r
            }
            _ => false,
        }
    }
}

impl<Tier> SyntaxEq for Case<Tier>
where
    Tier: SyntaxEq,
{
    fn syntax_eq(&self, other: &Self) -> bool {
        self.guard.syntax_eq(&other.guard) && self.block.syntax_eq(&other.block)
    }
}

impl<Tier> SyntaxEq for [Case<Tier>]
where
    Tier: SyntaxEq,
{
    fn syntax_eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .zip(other)
                .all(|(case_l, case_r)| case_l.syntax_eq(case_r))
    }
}

impl SyntaxEq for Guard {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Guard::Bool(value_l), Guard::Bool(value_r)) => value_l == value_r,
            (Guard::Cmp(op_l, typ_l, exp_l), Guard::Cmp(op_r, typ_r, exp_r)) => {
                op_l.syntax_eq(op_r) && typ_l.syntax_eq(typ_r) && exp_l.syntax_eq(exp_r)
            }
            (Guard::Sub(typ_l, _), Guard::Sub(typ_r, _)) => typ_l.syntax_eq(typ_r),
            (Guard::Match(pattern_l), Guard::Match(pattern_r)) => pattern_l.syntax_eq(pattern_r),
            (Guard::Mem(exp_l), Guard::Mem(exp_r)) => exp_l.syntax_eq(exp_r),
            (Guard::CheckLetSub(typ_l, _, exp_l), Guard::CheckLetSub(typ_r, _, exp_r)) => {
                typ_l.syntax_eq(typ_r) && exp_l.syntax_eq(exp_r)
            }
            (Guard::CheckLetMatch(pattern_l, exp_l), Guard::CheckLetMatch(pattern_r, exp_r)) => {
                pattern_l.syntax_eq(pattern_r) && exp_l.syntax_eq(exp_r)
            }
            _ => false,
        }
    }
}

impl SyntaxEq for Fallthrough {
    fn syntax_eq(&self, _other: &Self) -> bool {
        true
    }
}

impl SyntaxEq for InstrNote {
    fn syntax_eq(&self, _other: &Self) -> bool {
        true
    }
}

// - Instructions

impl<Tier> SyntaxEq for InstrNode<Tier>
where
    Tier: SyntaxEq,
{
    fn syntax_eq(&self, other: &Self) -> bool {
        self.node.kind.syntax_eq(&other.node.kind)
    }
}

impl<Tier> SyntaxEq for Instr<Tier>
where
    Tier: SyntaxEq,
{
    fn syntax_eq(&self, other: &Self) -> bool {
        self.node.syntax_eq(&other.node)
    }
}

impl<Tier> SyntaxEq for [Instr<Tier>]
where
    Tier: SyntaxEq,
{
    fn syntax_eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .zip(other)
                .all(|(instr_l, instr_r)| instr_l.syntax_eq(instr_r))
    }
}

impl<Tier> SyntaxEq for Block<Tier>
where
    Tier: SyntaxEq,
{
    fn syntax_eq(&self, other: &Self) -> bool {
        self.as_slice().syntax_eq(other.as_slice())
    }
}

impl<Tier> SyntaxEq for InstrKind<Tier>
where
    Tier: SyntaxEq,
{
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (InstrKind::If(instr_l), InstrKind::If(instr_r)) => instr_l.syntax_eq(instr_r),
            (InstrKind::Hold(instr_l), InstrKind::Hold(instr_r)) => instr_l.syntax_eq(instr_r),
            (InstrKind::Case(instr_l), InstrKind::Case(instr_r)) => instr_l.syntax_eq(instr_r),
            (InstrKind::Let(instr_l), InstrKind::Let(instr_r)) => instr_l.syntax_eq(instr_r),
            (InstrKind::Debug(instr_l), InstrKind::Debug(instr_r)) => instr_l.syntax_eq(instr_r),
            (InstrKind::Destruct(instr_l), InstrKind::Destruct(instr_r)) => {
                instr_l.syntax_eq(instr_r)
            }
            (InstrKind::CheckLetSub(instr_l), InstrKind::CheckLetSub(instr_r)) => {
                instr_l.syntax_eq(instr_r)
            }
            (InstrKind::CheckLetMatch(instr_l), InstrKind::CheckLetMatch(instr_r)) => {
                instr_l.syntax_eq(instr_r)
            }
            (InstrKind::OptionGet(instr_l), InstrKind::OptionGet(instr_r)) => {
                instr_l.syntax_eq(instr_r)
            }
            (InstrKind::Tier(instr_l), InstrKind::Tier(instr_r)) => instr_l.syntax_eq(instr_r),
            _ => false,
        }
    }
}

impl<Tier> SyntaxEq for IfInstr<Tier>
where
    Tier: SyntaxEq,
{
    fn syntax_eq(&self, other: &Self) -> bool {
        self.exp.syntax_eq(&other.exp)
            && self.iter_exps.syntax_eq(&other.iter_exps)
            && self.block.syntax_eq(&other.block)
            && self.dangle == other.dangle
    }
}

impl<Tier> SyntaxEq for HoldInstr<Tier>
where
    Tier: SyntaxEq,
{
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.not_exp.syntax_eq(&other.not_exp)
            && self.iter_exps.syntax_eq(&other.iter_exps)
            && self.hold_case.syntax_eq(&other.hold_case)
    }
}

impl<Tier> SyntaxEq for CaseInstr<Tier>
where
    Tier: SyntaxEq,
{
    fn syntax_eq(&self, other: &Self) -> bool {
        self.exp.syntax_eq(&other.exp)
            && self.cases.syntax_eq(&other.cases)
            && self.dangle == other.dangle
    }
}

impl SyntaxEq for LetInstr {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.exp_l.syntax_eq(&other.exp_l)
            && self.exp_r.syntax_eq(&other.exp_r)
            && self.iter_instrs.syntax_eq(&other.iter_instrs)
    }
}

impl SyntaxEq for DebugInstr {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.exp.syntax_eq(&other.exp)
    }
}

impl SyntaxEq for DestructInstr {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.bindings.len() == other.bindings.len()
            && self.bindings.iter().zip(&other.bindings).all(
                |((name_l, exp_l), (name_r, exp_r))| name_l == name_r && exp_l.syntax_eq(exp_r),
            )
            && self.exp.syntax_eq(&other.exp)
    }
}

impl<Tier> SyntaxEq for CheckLetSubInstr<Tier>
where
    Tier: SyntaxEq,
{
    fn syntax_eq(&self, other: &Self) -> bool {
        self.typ.syntax_eq(&other.typ)
            && self.exp_l.syntax_eq(&other.exp_l)
            && self.exp_r.syntax_eq(&other.exp_r)
            && self.block.syntax_eq(&other.block)
    }
}

impl<Tier> SyntaxEq for CheckLetMatchInstr<Tier>
where
    Tier: SyntaxEq,
{
    fn syntax_eq(&self, other: &Self) -> bool {
        self.pattern.syntax_eq(&other.pattern)
            && self.exp_l.syntax_eq(&other.exp_l)
            && self.exp_r.syntax_eq(&other.exp_r)
            && self.block.syntax_eq(&other.block)
    }
}

impl<Tier> SyntaxEq for OptionGetInstr<Tier>
where
    Tier: SyntaxEq,
{
    fn syntax_eq(&self, other: &Self) -> bool {
        self.exp_l.syntax_eq(&other.exp_l)
            && self.exp_r.syntax_eq(&other.exp_r)
            && self.block.syntax_eq(&other.block)
    }
}

impl<Tier> SyntaxEq for TierInstr<Tier>
where
    Tier: SyntaxEq,
{
    fn syntax_eq(&self, other: &Self) -> bool {
        self.tier.syntax_eq(&other.tier)
    }
}

// - Group-body tier

impl SyntaxEq for InstrGroup {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (InstrGroup::Result(instr_l), InstrGroup::Result(instr_r)) => {
                instr_l.syntax_eq(instr_r)
            }
            (InstrGroup::Return(instr_l), InstrGroup::Return(instr_r)) => {
                instr_l.syntax_eq(instr_r)
            }
            (InstrGroup::Rule(instr_l), InstrGroup::Rule(instr_r)) => instr_l.syntax_eq(instr_r),
            (InstrGroup::Backtrack(instr_l), InstrGroup::Backtrack(instr_r)) => {
                instr_l.syntax_eq(instr_r)
            }
            _ => false,
        }
    }
}

impl SyntaxEq for ResultGroupInstr {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.rel_signature.syntax_eq(&other.rel_signature)
            && self.exps_output.syntax_eq(&other.exps_output)
    }
}

impl SyntaxEq for ReturnGroupInstr {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.exp.syntax_eq(&other.exp)
    }
}

impl SyntaxEq for RuleGroupInstr {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.not_exp.syntax_eq(&other.not_exp)
            && self.input_hint == other.input_hint
            && self.iter_instrs.syntax_eq(&other.iter_instrs)
    }
}

impl SyntaxEq for BacktrackGroupInstr {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.blocks.syntax_eq(&other.blocks)
    }
}

impl SyntaxEq for [BlockGroup] {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .zip(other)
                .all(|(block_l, block_r)| block_l.syntax_eq(block_r))
    }
}

// - Dispatch tier

impl SyntaxEq for InstrDispatch {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (InstrDispatch::Group(instr_l), InstrDispatch::Group(instr_r)) => {
                instr_l.syntax_eq(instr_r)
            }
            (InstrDispatch::Route(instr_l), InstrDispatch::Route(instr_r)) => {
                instr_l.syntax_eq(instr_r)
            }
            _ => false,
        }
    }
}

impl SyntaxEq for GroupDispatchInstr {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id_rel.syntax_eq(&other.id_rel)
            && self.id_group.syntax_eq(&other.id_group)
            && self.rel_signature.syntax_eq(&other.rel_signature)
            && self.exps_input.syntax_eq(&other.exps_input)
            && self.block.syntax_eq(&other.block)
    }
}

impl SyntaxEq for RouteDispatchInstr {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.blocks.syntax_eq(&other.blocks)
    }
}

impl SyntaxEq for [BlockDispatch] {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .zip(other)
                .all(|(block_l, block_r)| block_l.syntax_eq(block_r))
    }
}

// - Relations and functions

impl SyntaxEq for ExternRel {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.rel_signature.syntax_eq(&other.rel_signature)
            && self.exps_input.syntax_eq(&other.exps_input)
    }
}

impl SyntaxEq for Rel {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.rel_signature.syntax_eq(&other.rel_signature)
            && self.exps_input.syntax_eq(&other.exps_input)
            && self.block.syntax_eq(&other.block)
            && match (&self.block_else_opt, &other.block_else_opt) {
                (Some(block_l), Some(block_r)) => block_l.syntax_eq(block_r),
                (None, None) => true,
                _ => false,
            }
    }
}

impl SyntaxEq for ExternFunc {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.tparams.syntax_eq(&other.tparams)
            && self.params.syntax_eq(&other.params)
            && self.typ.syntax_eq(&other.typ)
    }
}

impl SyntaxEq for BuiltinFunc {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.tparams.syntax_eq(&other.tparams)
            && self.params.syntax_eq(&other.params)
            && self.typ.syntax_eq(&other.typ)
    }
}

impl SyntaxEq for TableRow {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.exps_input.syntax_eq(&other.exps_input)
            && self.exp.syntax_eq(&other.exp)
            && self.block.syntax_eq(&other.block)
    }
}

impl SyntaxEq for [TableRow] {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .zip(other)
                .all(|(row_l, row_r)| row_l.syntax_eq(row_r))
    }
}

impl SyntaxEq for TableFunc {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.params.syntax_eq(&other.params)
            && self.typ.syntax_eq(&other.typ)
            && self.rows.syntax_eq(&other.rows)
    }
}

impl SyntaxEq for DefinedFunc {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.tparams.syntax_eq(&other.tparams)
            && self.params.syntax_eq(&other.params)
            && self.typ.syntax_eq(&other.typ)
            && self.block.syntax_eq(&other.block)
            && match (&self.block_else_opt, &other.block_else_opt) {
                (Some(block_l), Some(block_r)) => block_l.syntax_eq(block_r),
                (None, None) => true,
                _ => false,
            }
    }
}

// - Definitions

impl SyntaxEq for DefNode {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.node.syntax_eq(&other.node)
    }
}

impl SyntaxEq for ExternTypDef {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
    }
}

impl SyntaxEq for TypDef {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.tparams.syntax_eq(&other.tparams)
            && self.def_typ.syntax_eq(&other.def_typ)
    }
}

impl SyntaxEq for VarDef {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id) && self.typ.syntax_eq(&other.typ)
    }
}

impl SyntaxEq for DefKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (DefKind::ExternTyp(def_l), DefKind::ExternTyp(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::Typ(def_l), DefKind::Typ(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::Var(def_l), DefKind::Var(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::ExternRel(def_l), DefKind::ExternRel(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::Rel(def_l), DefKind::Rel(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::ExternDec(def_l), DefKind::ExternDec(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::BuiltinDec(def_l), DefKind::BuiltinDec(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::TableDec(def_l), DefKind::TableDec(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::FuncDec(def_l), DefKind::FuncDec(def_r)) => def_l.syntax_eq(def_r),
            _ => false,
        }
    }
}

impl SyntaxEq for Def {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.node.syntax_eq(&other.node)
    }
}

impl SyntaxEq for [Def] {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .zip(other)
                .all(|(def_l, def_r)| def_l.syntax_eq(def_r))
    }
}

impl SyntaxEq for Spec {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.as_slice().syntax_eq(other.as_slice())
    }
}
