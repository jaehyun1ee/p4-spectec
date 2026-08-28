//! Free identifiers in elaboration-language data

use crate::lang::{common::ds::set::IdSet, traits::free::Free};

use super::ast::*;

// == Free identifiers

// - Iterators

impl Free for Iter {
    fn collect_free(&self, _free: &mut IdSet) {}
}

// - Types

impl Free for PlainTypKind {
    fn collect_free(&self, _free: &mut IdSet) {}
}

// - Operators

impl Free for NumOp {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for UnOp {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for BinOp {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for CmpOp {
    fn collect_free(&self, _free: &mut IdSet) {}
}

// - Expressions

impl Free for ExpKind {
    fn collect_free(&self, free: &mut IdSet) {
        match self {
            Self::Bool(_)
            | Self::Num(_, _)
            | Self::Text(_)
            | Self::Eps
            | Self::Atom(_)
            | Self::Hole(_)
            | Self::Latex(_) => {}
            Self::Var(id) => {
                free.insert(id.clone());
            }
            Self::Un(_, exp)
            | Self::Arith(exp)
            | Self::Len(exp)
            | Self::Dot(exp, _)
            | Self::Paren(exp)
            | Self::Iter(exp, _)
            | Self::Sub(exp, _)
            | Self::Brack(_, exp, _)
            | Self::Unparen(exp) => exp.collect_free(free),
            Self::Bin(exp_l, _, exp_r)
            | Self::Cmp(exp_l, _, exp_r)
            | Self::Cons(exp_l, exp_r)
            | Self::Cat(exp_l, exp_r)
            | Self::Idx(exp_l, exp_r)
            | Self::Mem(exp_l, exp_r)
            | Self::Infix(exp_l, _, exp_r)
            | Self::Fuse(exp_l, exp_r) => {
                exp_l.collect_free(free);
                exp_r.collect_free(free);
            }
            Self::List(exps) | Self::Tuple(exps) | Self::Seq(exps) => {
                exps.as_slice().collect_free(free);
            }
            Self::Slice(exp_b, exp_i, exp_n) => {
                exp_b.collect_free(free);
                exp_i.collect_free(free);
                exp_n.collect_free(free);
            }
            Self::Str(fields) => {
                for (_, exp) in fields {
                    exp.collect_free(free);
                }
            }
            Self::Upd(exp_b, path, exp_f) => {
                exp_b.collect_free(free);
                path.collect_free(free);
                exp_f.collect_free(free);
            }
            Self::Call(_, _, args) => args.as_slice().collect_free(free),
        }
    }
}

impl Free for Hole {
    fn collect_free(&self, _free: &mut IdSet) {}
}

// - Paths

impl Free for PathKind {
    fn collect_free(&self, free: &mut IdSet) {
        match self {
            Self::Root => {}
            Self::Idx(path, exp) => {
                path.collect_free(free);
                exp.collect_free(free);
            }
            Self::Slice(path, exp_i, exp_n) => {
                path.collect_free(free);
                exp_i.collect_free(free);
                exp_n.collect_free(free);
            }
            Self::Dot(path, _) => path.collect_free(free),
        }
    }
}

// - Arguments

impl Free for ArgKind {
    fn collect_free(&self, free: &mut IdSet) {
        match self {
            Self::Exp(exp) => exp.collect_free(free),
            Self::Def(_) => {}
        }
    }
}

// - Type arguments

// `Targ` aliases `PlainTyp` and uses its implementation above.

// - Hints

impl Free for Hint {
    fn collect_free(&self, free: &mut IdSet) {
        self.1.collect_free(free);
    }
}

// - Notation types

impl Free for Typ {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for NotTypKind {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for DefTypKind {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for TypField {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for TypCase {
    fn collect_free(&self, _free: &mut IdSet) {}
}

// - Parameters and premises

impl Free for ParamKind {
    fn collect_free(&self, _free: &mut IdSet) {}
}

// `TParam` aliases `Id` and uses its implementation above.

impl Free for VarPrem {
    fn collect_free(&self, free: &mut IdSet) {
        free.insert(self.id.clone());
    }
}

impl Free for RulePrem {
    fn collect_free(&self, free: &mut IdSet) {
        self.exp.collect_free(free);
    }
}

impl Free for RuleNotPrem {
    fn collect_free(&self, free: &mut IdSet) {
        self.exp.collect_free(free);
    }
}

impl Free for IfPrem {
    fn collect_free(&self, free: &mut IdSet) {
        self.exp.collect_free(free);
    }
}

impl Free for IterPrem {
    fn collect_free(&self, free: &mut IdSet) {
        self.prem.collect_free(free);
    }
}

impl Free for DebugPrem {
    fn collect_free(&self, free: &mut IdSet) {
        self.exp.collect_free(free);
    }
}

impl Free for PremKind {
    fn collect_free(&self, free: &mut IdSet) {
        match self {
            Self::Var(prem) => prem.collect_free(free),
            Self::Rule(prem) => prem.collect_free(free),
            Self::RuleNot(prem) => prem.collect_free(free),
            Self::If(prem) => prem.collect_free(free),
            Self::Else => {}
            Self::Iter(prem) => prem.collect_free(free),
            Self::Debug(prem) => prem.collect_free(free),
        }
    }
}

// - Rules and tables

impl Free for RuleKind {
    fn collect_free(&self, free: &mut IdSet) {
        self.2.collect_free(free);
        self.3.as_slice().collect_free(free);
    }
}

impl Free for TableRowKind {
    fn collect_free(&self, free: &mut IdSet) {
        self.0.collect_free(free);
        self.1.collect_free(free);
    }
}

// - Definitions

impl Free for ExternSyntaxDef {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for SyntaxDef {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for SyntaxDefEntry {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for TypDef {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for VarDef {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for ExternRelDef {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for RelDef {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for RuleGroupDef {
    fn collect_free(&self, free: &mut IdSet) {
        self.rules.as_slice().collect_free(free);
    }
}

impl Free for ExternDecDef {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for BuiltinDecDef {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for TableDecDef {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for FuncDecDef {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for TableDef {
    fn collect_free(&self, free: &mut IdSet) {
        self.rows.as_slice().collect_free(free);
    }
}

impl Free for FuncDef {
    fn collect_free(&self, free: &mut IdSet) {
        self.args.as_slice().collect_free(free);
        self.exp.collect_free(free);
        self.prems.as_slice().collect_free(free);
    }
}

impl Free for DefKind {
    fn collect_free(&self, free: &mut IdSet) {
        match self {
            Self::ExternSyntax(definition) => definition.collect_free(free),
            Self::Syntax(definition) => definition.collect_free(free),
            Self::Typ(definition) => definition.collect_free(free),
            Self::Var(definition) => definition.collect_free(free),
            Self::ExternRel(definition) => definition.collect_free(free),
            Self::Rel(definition) => definition.collect_free(free),
            Self::RuleGroup(definition) => definition.collect_free(free),
            Self::ExternDec(definition) => definition.collect_free(free),
            Self::BuiltinDec(definition) => definition.collect_free(free),
            Self::TableDec(definition) => definition.collect_free(free),
            Self::FuncDec(definition) => definition.collect_free(free),
            Self::TableDef(definition) => definition.collect_free(free),
            Self::FuncDef(definition) => definition.collect_free(free),
            Self::Sep => {}
        }
    }
}

// - Specifications

impl Free for Spec {
    fn collect_free(&self, free: &mut IdSet) {
        self.as_slice().collect_free(free);
    }
}
