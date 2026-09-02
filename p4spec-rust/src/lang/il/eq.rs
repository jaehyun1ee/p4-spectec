//! Syntax equality for intermediate-language data
//!
//! Ignores source regions;
//! compares source-annotated syntax by node contents

use crate::lang::traits::eq::SyntaxEq;

use super::ast::*;

// == Syntax equality

// - Iterators

impl SyntaxEq for Iter {
    fn syntax_eq(&self, other: &Self) -> bool {
        self == other
    }
}

// - Variables

impl SyntaxEq for Var {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id) && self.iters == other.iters
    }

    fn slice_syntax_eq(vars_l: &[Self], vars_r: &[Self]) -> bool {
        let mut vars_l = vars_l.iter().collect::<Vec<_>>();
        let mut vars_r = vars_r.iter().collect::<Vec<_>>();
        let cmp_var = |var_l: &&Var, var_r: &&Var| {
            var_l
                .id
                .node
                .cmp(&var_r.id.node)
                .then_with(|| var_l.iters.cmp(&var_r.iters))
        };
        vars_l.sort_by(cmp_var);
        vars_r.sort_by(cmp_var);
        vars_l.len() == vars_r.len()
            && vars_l
                .into_iter()
                .zip(vars_r)
                .all(|(var_l, var_r)| var_l.syntax_eq(var_r))
    }
}

// - Types

impl SyntaxEq for TypKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (TypKind::Bool, TypKind::Bool) | (TypKind::Text, TypKind::Text) => true,
            (TypKind::Num(num_typ_l), TypKind::Num(num_typ_r)) => num_typ_l == num_typ_r,
            (TypKind::Var(id_l, targs_l), TypKind::Var(id_r, targs_r)) => {
                id_l.syntax_eq(id_r) && targs_l.syntax_eq(targs_r)
            }
            (TypKind::Tuple(typs_l), TypKind::Tuple(typs_r)) => typs_l.syntax_eq(typs_r),
            (TypKind::Iter(typ_l, iter_l), TypKind::Iter(typ_r, iter_r)) => {
                typ_l.syntax_eq(typ_r) && iter_l == iter_r
            }
            (TypKind::Func(func_typ_l), TypKind::Func(func_typ_r)) => {
                func_typ_l.syntax_eq(func_typ_r)
            }
            _ => false,
        }
    }
}

impl SyntaxEq for FuncTyp {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.tparams.syntax_eq(&other.tparams)
            && self.typs_params.syntax_eq(&other.typs_params)
            && self.typ_ret.syntax_eq(&other.typ_ret)
    }
}

// - Values

impl SyntaxEq for ValueKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ValueKind::Bool(value_l), ValueKind::Bool(value_r)) => value_l == value_r,
            (ValueKind::Num(value_l), ValueKind::Num(value_r)) => value_l == value_r,
            (ValueKind::Text(value_l), ValueKind::Text(value_r)) => value_l == value_r,
            (ValueKind::Struct(fields_l), ValueKind::Struct(fields_r)) => {
                fields_l.len() == fields_r.len()
                    && fields_l.iter().zip(fields_r).all(
                        |((atom_l, value_l), (atom_r, value_r))| {
                            atom_l.syntax_eq(atom_r) && value_l.syntax_eq(value_r)
                        },
                    )
            }
            (ValueKind::Case(value_l), ValueKind::Case(value_r)) => {
                value_l.eq_by(value_r, SyntaxEq::syntax_eq)
            }
            (ValueKind::Tuple(values_l), ValueKind::Tuple(values_r))
            | (ValueKind::List(values_l), ValueKind::List(values_r)) => {
                values_l.syntax_eq(values_r)
            }
            (ValueKind::Opt(Some(value_l)), ValueKind::Opt(Some(value_r))) => {
                value_l.syntax_eq(value_r)
            }
            (ValueKind::Opt(None), ValueKind::Opt(None)) => true,
            (ValueKind::Func(id_l), ValueKind::Func(id_r)) => id_l == id_r,
            (ValueKind::Extern(value_l), ValueKind::Extern(value_r)) => value_l == value_r,
            _ => false,
        }
    }
}

impl SyntaxEq for ValueField {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.0.syntax_eq(&other.0) && self.1.syntax_eq(&other.1)
    }
}

// - Expressions

impl SyntaxEq for ExpKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ExpKind::Bool(value_l), ExpKind::Bool(value_r)) => value_l == value_r,
            (ExpKind::Num(value_l), ExpKind::Num(value_r)) => value_l == value_r,
            (ExpKind::Text(value_l), ExpKind::Text(value_r)) => value_l == value_r,
            (ExpKind::Var(id_l), ExpKind::Var(id_r)) => id_l.syntax_eq(id_r),
            (ExpKind::Un(op_l, typ_l, exp_l), ExpKind::Un(op_r, typ_r, exp_r)) => {
                op_l == op_r && typ_l == typ_r && exp_l.syntax_eq(exp_r)
            }
            (
                ExpKind::Bin(op_l, typ_l, exp_l_l, exp_r_l),
                ExpKind::Bin(op_r, typ_r, exp_l_r, exp_r_r),
            ) => {
                op_l == op_r
                    && typ_l == typ_r
                    && exp_l_l.syntax_eq(exp_l_r)
                    && exp_r_l.syntax_eq(exp_r_r)
            }
            (
                ExpKind::Cmp(op_l, typ_l, exp_l_l, exp_r_l),
                ExpKind::Cmp(op_r, typ_r, exp_l_r, exp_r_r),
            ) => {
                op_l == op_r
                    && typ_l == typ_r
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
            (ExpKind::Case(not_exp_l), ExpKind::Case(not_exp_r)) => {
                not_exp_l.eq_by(not_exp_r, SyntaxEq::syntax_eq)
            }
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

impl SyntaxEq for IterExp {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.0 == other.0 && self.1.syntax_eq(&other.1)
    }
}

// - Patterns

impl SyntaxEq for Pattern {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Pattern::Case(mixop_l), Pattern::Case(mixop_r)) => mixop_l == mixop_r,
            (Pattern::List(pattern_l), Pattern::List(pattern_r)) => pattern_l == pattern_r,
            (Pattern::Opt(pattern_l), Pattern::Opt(pattern_r)) => pattern_l == pattern_r,
            _ => false,
        }
    }
}

// - Paths

impl SyntaxEq for PathKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PathKind::Root, PathKind::Root) => true,
            (PathKind::Idx(path_l, exp_l), PathKind::Idx(path_r, exp_r)) => {
                path_l.syntax_eq(path_r) && exp_l.syntax_eq(exp_r)
            }
            (
                PathKind::Slice(path_l, exp_i_l, exp_n_l),
                PathKind::Slice(path_r, exp_i_r, exp_n_r),
            ) => {
                path_l.syntax_eq(path_r) && exp_i_l.syntax_eq(exp_i_r) && exp_n_l.syntax_eq(exp_n_r)
            }
            (PathKind::Dot(path_l, atom_l), PathKind::Dot(path_r, atom_r)) => {
                path_l.syntax_eq(path_r) && atom_l.syntax_eq(atom_r)
            }
            _ => false,
        }
    }
}

// - Arguments

impl SyntaxEq for ArgKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ArgKind::Exp(exp_l), ArgKind::Exp(exp_r)) => exp_l.syntax_eq(exp_r),
            (ArgKind::Def(id_l), ArgKind::Def(id_r)) => id_l.syntax_eq(id_r),
            _ => false,
        }
    }
}

// - Premises

impl SyntaxEq for PremKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PremKind::Rule(prem_l), PremKind::Rule(prem_r)) => prem_l.syntax_eq(prem_r),
            (PremKind::If(prem_l), PremKind::If(prem_r)) => prem_l.syntax_eq(prem_r),
            (PremKind::IfHold(prem_l), PremKind::IfHold(prem_r)) => prem_l.syntax_eq(prem_r),
            (PremKind::IfNotHold(prem_l), PremKind::IfNotHold(prem_r)) => prem_l.syntax_eq(prem_r),
            (PremKind::Iter(prem_l), PremKind::Iter(prem_r)) => prem_l.syntax_eq(prem_r),
            (PremKind::Debug(prem_l), PremKind::Debug(prem_r)) => prem_l.syntax_eq(prem_r),
            _ => false,
        }
    }
}
impl SyntaxEq for PremIter {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.iter.syntax_eq(&other.iter)
            && self.vars_bound.syntax_eq(&other.vars_bound)
            && self.vars_bind.syntax_eq(&other.vars_bind)
    }
}

// - Subtype checks

impl SyntaxEq for Subcheck {
    fn syntax_eq(&self, _other: &Self) -> bool {
        true
    }

    fn slice_syntax_eq(_subchecks_l: &[Self], _subchecks_r: &[Self]) -> bool {
        true
    }
}

// - Defined types

impl SyntaxEq for DefTypKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (DefTypKind::Plain(typ_l), DefTypKind::Plain(typ_r)) => typ_l.syntax_eq(typ_r),
            (DefTypKind::Struct(fields_l), DefTypKind::Struct(fields_r)) => {
                fields_l.syntax_eq(fields_r)
            }
            (DefTypKind::Variant(cases_l), DefTypKind::Variant(cases_r)) => {
                cases_l.syntax_eq(cases_r)
            }
            _ => false,
        }
    }
}

impl SyntaxEq for TypField {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.0.syntax_eq(&other.0) && self.1.syntax_eq(&other.1)
    }
}

impl SyntaxEq for TypOriginKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.0.syntax_eq(&other.0) && self.1.syntax_eq(&other.1)
    }
}

impl SyntaxEq for TypCase {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.0.syntax_eq(&other.0) && self.1.syntax_eq(&other.1) && self.2.syntax_eq(&other.2)
    }
}

impl SyntaxEq for OpTyp {
    fn syntax_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl SyntaxEq for ListPattern {
    fn syntax_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl SyntaxEq for OptPattern {
    fn syntax_eq(&self, other: &Self) -> bool {
        self == other
    }
}

// - Parameters

impl SyntaxEq for ParamKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ParamKind::Exp(typ_l), ParamKind::Exp(typ_r)) => typ_l.syntax_eq(typ_r),
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

// - Premise payloads

impl SyntaxEq for RulePrem {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.not_exp.syntax_eq(&other.not_exp)
            && self.input_hint == other.input_hint
    }
}

impl SyntaxEq for IfPrem {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.exp.syntax_eq(&other.exp)
    }
}

impl SyntaxEq for IfHoldPrem {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id) && self.not_exp.syntax_eq(&other.not_exp)
    }
}

impl SyntaxEq for IfNotHoldPrem {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id) && self.not_exp.syntax_eq(&other.not_exp)
    }
}

impl SyntaxEq for IterPrem {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.prem.syntax_eq(&other.prem) && self.prem_iter.syntax_eq(&other.prem_iter)
    }
}

impl SyntaxEq for DebugPrem {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.exp.syntax_eq(&other.exp)
    }
}

// - Rules and clauses

impl SyntaxEq for RuleKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.not_exp.syntax_eq(&other.not_exp)
            && self.prems.syntax_eq(&other.prems)
    }
}

impl SyntaxEq for RuleGroupKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.0.syntax_eq(&other.0) && self.1.syntax_eq(&other.1)
    }
}

impl SyntaxEq for ElseGroupKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.0.syntax_eq(&other.0) && self.1.syntax_eq(&other.1)
    }
}

impl SyntaxEq for ClauseKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.args.syntax_eq(&other.args)
            && self.expression.syntax_eq(&other.expression)
            && self.premises.syntax_eq(&other.premises)
    }
}

impl SyntaxEq for TableRowKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.0.syntax_eq(&other.0) && self.1.syntax_eq(&other.1)
    }
}

// - Definitions

impl SyntaxEq for ExternTyp {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id) && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for TypDef {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.tparams.syntax_eq(&other.tparams)
            && self.def_typ.syntax_eq(&other.def_typ)
            && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for VarDef {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.typ.syntax_eq(&other.typ)
            && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for ExternRel {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.not_typ.syntax_eq(&other.not_typ)
            && self.input_hint == other.input_hint
            && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for Rel {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.not_typ.syntax_eq(&other.not_typ)
            && self.input_hint == other.input_hint
            && self.rule_groups.syntax_eq(&other.rule_groups)
            && match (&self.else_group, &other.else_group) {
                (Some(group_l), Some(group_r)) => group_l.syntax_eq(group_r),
                (None, None) => true,
                _ => false,
            }
            && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for ExternDec {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.tparams.syntax_eq(&other.tparams)
            && self.params.syntax_eq(&other.params)
            && self.typ.syntax_eq(&other.typ)
            && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for BuiltinDec {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.tparams.syntax_eq(&other.tparams)
            && self.params.syntax_eq(&other.params)
            && self.typ.syntax_eq(&other.typ)
            && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for TableDec {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.params.syntax_eq(&other.params)
            && self.typ.syntax_eq(&other.typ)
            && self.rows.syntax_eq(&other.rows)
            && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for FuncDec {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.tparams.syntax_eq(&other.tparams)
            && self.params.syntax_eq(&other.params)
            && self.typ.syntax_eq(&other.typ)
            && self.clauses.syntax_eq(&other.clauses)
            && match (&self.else_clause, &other.else_clause) {
                (Some(clause_l), Some(clause_r)) => clause_l.syntax_eq(clause_r),
                (None, None) => true,
                _ => false,
            }
            && self.hints.syntax_eq(&other.hints)
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

// - Specifications

impl SyntaxEq for Spec {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.as_slice().syntax_eq(other.as_slice())
    }
}
