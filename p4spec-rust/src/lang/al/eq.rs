//! Syntax equality for algorithmic-language data
//!
//! Reuses IL equality for aliases and compares AL-specific rule structure

use crate::lang::traits::eq::SyntaxEq;

use super::ast::*;

// == Syntax equality

impl SyntaxEq for RuleMatch {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.exps_signature.syntax_eq(&other.exps_signature)
            && self.exps_input.syntax_eq(&other.exps_input)
            && self.prems.syntax_eq(&other.prems)
    }
}

impl SyntaxEq for RulePath {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.prems.syntax_eq(&other.prems)
            && self.exps_output.syntax_eq(&other.exps_output)
    }
}

impl SyntaxEq for RuleGroupKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.rule_match.syntax_eq(&other.rule_match)
            && self.rule_paths.syntax_eq(&other.rule_paths)
    }
}

impl SyntaxEq for ElseGroupKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.rule_match.syntax_eq(&other.rule_match)
            && self.rule_path.syntax_eq(&other.rule_path)
    }
}

impl SyntaxEq for TableRowKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.exps_signature.syntax_eq(&other.exps_signature)
            && self.args.syntax_eq(&other.args)
            && self.exp.syntax_eq(&other.exp)
            && self.prems.syntax_eq(&other.prems)
    }
}

impl SyntaxEq for ExternTypDef {
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

impl SyntaxEq for ExternRelDef {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.not_typ.syntax_eq(&other.not_typ)
            && self.input_hint == other.input_hint
            && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for RelDef {
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

impl SyntaxEq for ExternDecDef {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.tparams.syntax_eq(&other.tparams)
            && self.params.syntax_eq(&other.params)
            && self.typ.syntax_eq(&other.typ)
            && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for BuiltinDecDef {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.tparams.syntax_eq(&other.tparams)
            && self.params.syntax_eq(&other.params)
            && self.typ.syntax_eq(&other.typ)
            && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for TableDecDef {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.params.syntax_eq(&other.params)
            && self.typ.syntax_eq(&other.typ)
            && self.table_rows.syntax_eq(&other.table_rows)
            && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for FuncDecDef {
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

impl SyntaxEq for Spec {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.as_slice().syntax_eq(other.as_slice())
    }
}
