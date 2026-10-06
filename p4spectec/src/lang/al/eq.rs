//! Syntax equality for algorithmic-language data
//!
//! Reuses IL equality for the aliased items
//! and compares the AL-specific premises, matches, paths, and clauses
//! structurally, ignoring source regions.

use std::rc::Rc;

use crate::lang::traits::eq::SyntaxEq;

use super::ast::*;

// == Syntax equality

// - Premises

impl<P: Stage<Id: SyntaxEq, Var: SyntaxEq, Mixop = Rc<MixopTree>>> SyntaxEq for PremKind<P> {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PremKind::Rule(prem_l), PremKind::Rule(prem_r)) => prem_l.syntax_eq(prem_r),
            (PremKind::If(prem_l), PremKind::If(prem_r)) => prem_l.syntax_eq(prem_r),
            (PremKind::IfHold(prem_l), PremKind::IfHold(prem_r)) => prem_l.syntax_eq(prem_r),
            (PremKind::IfNotHold(prem_l), PremKind::IfNotHold(prem_r)) => prem_l.syntax_eq(prem_r),
            (PremKind::Let(prem_l), PremKind::Let(prem_r)) => prem_l.syntax_eq(prem_r),
            (PremKind::Iter(prem_l), PremKind::Iter(prem_r)) => prem_l.syntax_eq(prem_r),
            (PremKind::Debug(prem_l), PremKind::Debug(prem_r)) => prem_l.syntax_eq(prem_r),
            _ => false,
        }
    }
}

impl<P: Stage<Id: SyntaxEq, Var: SyntaxEq, Mixop = Rc<MixopTree>>> SyntaxEq for RulePrem<P> {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.not_exp.syntax_eq(&other.not_exp)
            && self.input_hint == other.input_hint
    }
}

impl<P: Stage<Id: SyntaxEq, Var: SyntaxEq, Mixop = Rc<MixopTree>>> SyntaxEq for IfPrem<P> {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.exp.syntax_eq(&other.exp)
    }
}

impl<P: Stage<Id: SyntaxEq, Var: SyntaxEq, Mixop = Rc<MixopTree>>> SyntaxEq for IfHoldPrem<P> {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id) && self.not_exp.syntax_eq(&other.not_exp)
    }
}

impl<P: Stage<Id: SyntaxEq, Var: SyntaxEq, Mixop = Rc<MixopTree>>> SyntaxEq for IfNotHoldPrem<P> {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id) && self.not_exp.syntax_eq(&other.not_exp)
    }
}

impl<P: Stage<Id: SyntaxEq, Var: SyntaxEq, Mixop = Rc<MixopTree>>> SyntaxEq for LetPrem<P> {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.exp_l.syntax_eq(&other.exp_l) && self.exp_r.syntax_eq(&other.exp_r)
    }
}

impl<P: Stage<Id: SyntaxEq, Var: SyntaxEq, Mixop = Rc<MixopTree>>> SyntaxEq for IterPrem<P> {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.prem.syntax_eq(&other.prem) && self.prem_iter.syntax_eq(&other.prem_iter)
    }
}

impl<P: Stage<Id: SyntaxEq, Var: SyntaxEq, Mixop = Rc<MixopTree>>> SyntaxEq for DebugPrem<P> {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.exp.syntax_eq(&other.exp)
    }
}

// - Rules

impl<P: Stage<Id: SyntaxEq, Var: SyntaxEq, Mixop = Rc<MixopTree>>> SyntaxEq for RuleGroupKind<P> {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.rule_match.syntax_eq(&other.rule_match)
            && self.rule_paths.syntax_eq(&other.rule_paths)
    }
}

impl<P: Stage<Id: SyntaxEq, Var: SyntaxEq, Mixop = Rc<MixopTree>>> SyntaxEq for ElseGroupKind<P> {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.rule_match.syntax_eq(&other.rule_match)
            && self.rule_path.syntax_eq(&other.rule_path)
    }
}

impl<P: Stage<Id: SyntaxEq, Var: SyntaxEq, Mixop = Rc<MixopTree>>> SyntaxEq for RuleMatch<P> {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.exps_signature.syntax_eq(&other.exps_signature)
            && self.exps_input.syntax_eq(&other.exps_input)
            && self.prems.syntax_eq(&other.prems)
    }
}

impl<P: Stage<Id: SyntaxEq, Var: SyntaxEq, Mixop = Rc<MixopTree>>> SyntaxEq for RulePath<P> {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.prems.syntax_eq(&other.prems)
            && self.exps_output.syntax_eq(&other.exps_output)
    }
}

// - Clauses

impl<P: Stage<Id: SyntaxEq, Var: SyntaxEq, Mixop = Rc<MixopTree>>> SyntaxEq for ClauseKind<P> {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.args.syntax_eq(&other.args)
            && self.exp.syntax_eq(&other.exp)
            && self.prems.syntax_eq(&other.prems)
    }
}

// - Table rows

impl<P: Stage<Id: SyntaxEq, Var: SyntaxEq, Mixop = Rc<MixopTree>>> SyntaxEq for TableRowKind<P> {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.exps_signature.syntax_eq(&other.exps_signature)
            && self.args.syntax_eq(&other.args)
            && self.exp.syntax_eq(&other.exp)
            && self.prems.syntax_eq(&other.prems)
    }
}

// == Type definitions

impl SyntaxEq for TypDef {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Extern(extern_typ_l), Self::Extern(extern_typ_r)) => {
                extern_typ_l.syntax_eq(extern_typ_r)
            }
            (Self::Defined(defined_typ_l), Self::Defined(defined_typ_r)) => {
                defined_typ_l.syntax_eq(defined_typ_r)
            }
            _ => false,
        }
    }
}

impl SyntaxEq for ExternTyp {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id) && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for DefinedTyp {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.tparams.syntax_eq(&other.tparams)
            && self.def_typ.syntax_eq(&other.def_typ)
            && self.hints.syntax_eq(&other.hints)
    }
}

// == Meta-variable definitions

impl SyntaxEq for VarDef {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.typ.syntax_eq(&other.typ)
            && self.hints.syntax_eq(&other.hints)
    }
}

// == Relation definitions

impl<P: Stage<Id: SyntaxEq, Var: SyntaxEq, Mixop = Rc<MixopTree>>> SyntaxEq for RelDef<P> {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Extern(extern_rel_l), Self::Extern(extern_rel_r)) => {
                extern_rel_l.syntax_eq(extern_rel_r)
            }
            (Self::Defined(defined_rel_l), Self::Defined(defined_rel_r)) => {
                defined_rel_l.syntax_eq(defined_rel_r)
            }
            _ => false,
        }
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

impl<P: Stage<Id: SyntaxEq, Var: SyntaxEq, Mixop = Rc<MixopTree>>> SyntaxEq for DefinedRel<P> {
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

// == Meta-function definitions

impl<P: Stage<Id: SyntaxEq, Var: SyntaxEq, Mixop = Rc<MixopTree>>> SyntaxEq for MetaFuncDef<P> {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Extern(extern_func_l), Self::Extern(extern_func_r)) => {
                extern_func_l.syntax_eq(extern_func_r)
            }
            (Self::Builtin(builtin_func_l), Self::Builtin(builtin_func_r)) => {
                builtin_func_l.syntax_eq(builtin_func_r)
            }
            (Self::Table(table_func_l), Self::Table(table_func_r)) => {
                table_func_l.syntax_eq(table_func_r)
            }
            (Self::Defined(defined_func_l), Self::Defined(defined_func_r)) => {
                defined_func_l.syntax_eq(defined_func_r)
            }
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
            && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for BuiltinFunc {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.tparams.syntax_eq(&other.tparams)
            && self.params.syntax_eq(&other.params)
            && self.typ.syntax_eq(&other.typ)
            && self.hints.syntax_eq(&other.hints)
    }
}

impl<P: Stage<Id: SyntaxEq, Var: SyntaxEq, Mixop = Rc<MixopTree>>> SyntaxEq for TableFunc<P> {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.params.syntax_eq(&other.params)
            && self.typ.syntax_eq(&other.typ)
            && self.table_rows.syntax_eq(&other.table_rows)
            && self.hints.syntax_eq(&other.hints)
    }
}

impl<P: Stage<Id: SyntaxEq, Var: SyntaxEq, Mixop = Rc<MixopTree>>> SyntaxEq for DefinedFunc<P> {
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

// == Definitions

impl<P: Stage<Id: SyntaxEq, Var: SyntaxEq, Mixop = Rc<MixopTree>>> SyntaxEq for DefKind<P> {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (DefKind::Typ(typ_def_l), DefKind::Typ(typ_def_r)) => typ_def_l.syntax_eq(typ_def_r),
            (DefKind::Var(var_def_l), DefKind::Var(var_def_r)) => var_def_l.syntax_eq(var_def_r),
            (DefKind::Rel(rel_def_l), DefKind::Rel(rel_def_r)) => rel_def_l.syntax_eq(rel_def_r),
            (DefKind::MetaFunc(meta_func_def_l), DefKind::MetaFunc(meta_func_def_r)) => {
                meta_func_def_l.syntax_eq(meta_func_def_r)
            }
            _ => false,
        }
    }
}

// == Specifications

impl<P: Stage<Id: SyntaxEq, Var: SyntaxEq, Mixop = Rc<MixopTree>>> SyntaxEq for Spec<P> {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.as_slice().syntax_eq(other.as_slice())
    }
}
