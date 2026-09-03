//! Free identifiers in elaboration-language data

use crate::lang::{common::ds::set::IdSet, traits::free::Free};

use super::ast::*;

// == Free identifiers

// - Types

impl Free for PlainTypKind {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

// - Operators

impl Free for NumOp {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for UnOp {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for BinOp {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for CmpOp {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

// - Expressions

impl Free for ExpKind {
    fn free_into(&self, free: &mut IdSet) {
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
            | Self::Unparen(exp) => exp.free_into(free),
            Self::Bin(exp_l, _, exp_r)
            | Self::Cmp(exp_l, _, exp_r)
            | Self::Cons(exp_l, exp_r)
            | Self::Cat(exp_l, exp_r)
            | Self::Idx(exp_l, exp_r)
            | Self::Mem(exp_l, exp_r)
            | Self::Infix(exp_l, _, exp_r)
            | Self::Fuse(exp_l, exp_r) => {
                exp_l.free_into(free);
                exp_r.free_into(free);
            }
            Self::List(exps) | Self::Tuple(exps) | Self::Seq(exps) => {
                exps.as_slice().free_into(free);
            }
            Self::Slice(exp_b, exp_i, exp_n) => {
                exp_b.free_into(free);
                exp_i.free_into(free);
                exp_n.free_into(free);
            }
            Self::Str(fields) => {
                for (_, exp) in fields {
                    exp.free_into(free);
                }
            }
            Self::Upd(exp_b, path, exp_f) => {
                exp_b.free_into(free);
                path.free_into(free);
                exp_f.free_into(free);
            }
            Self::Call(_, _, args) => args.as_slice().free_into(free),
        }
    }
}

impl Free for Hole {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

// - Paths

impl Free for PathKind {
    fn free_into(&self, free: &mut IdSet) {
        match self {
            Self::Root => {}
            Self::Idx(path, exp) => {
                path.free_into(free);
                exp.free_into(free);
            }
            Self::Slice(path, exp_i, exp_n) => {
                path.free_into(free);
                exp_i.free_into(free);
                exp_n.free_into(free);
            }
            Self::Dot(path, _) => path.free_into(free),
        }
    }
}

// - Arguments

impl Free for ArgKind {
    fn free_into(&self, free: &mut IdSet) {
        match self {
            Self::Exp(exp) => exp.free_into(free),
            Self::Def(_) => {}
        }
    }
}

// - Type arguments

// `Targ` aliases `PlainTyp` and uses its implementation above.

// - Hints

impl Free for Hint {
    fn free(&self) -> IdSet {
        self.1.free()
    }
}

// - Notation types

impl Free for Typ {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for NotTypKind {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for DefTypKind {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for TypField {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for TypCase {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

// - Parameters and premises

impl Free for ParamKind {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

// `TParam` aliases `Id` and uses its implementation above.

impl Free for VarPrem {
    fn free_into(&self, free: &mut IdSet) {
        free.insert(self.id.clone());
    }
}

impl Free for RulePrem {
    fn free_into(&self, free: &mut IdSet) {
        self.exp.free_into(free);
    }
}

impl Free for RuleNotPrem {
    fn free_into(&self, free: &mut IdSet) {
        self.exp.free_into(free);
    }
}

impl Free for IfPrem {
    fn free_into(&self, free: &mut IdSet) {
        self.exp.free_into(free);
    }
}

impl Free for IterPrem {
    fn free_into(&self, free: &mut IdSet) {
        self.prem.free_into(free);
    }
}

impl Free for DebugPrem {
    fn free_into(&self, free: &mut IdSet) {
        self.exp.free_into(free);
    }
}

impl Free for PremKind {
    fn free_into(&self, free: &mut IdSet) {
        match self {
            Self::Var(prem) => prem.free_into(free),
            Self::Rule(prem) => prem.free_into(free),
            Self::RuleNot(prem) => prem.free_into(free),
            Self::If(prem) => prem.free_into(free),
            Self::Else => {}
            Self::Iter(prem) => prem.free_into(free),
            Self::Debug(prem) => prem.free_into(free),
        }
    }
}

// - Rules and tables

impl Free for RuleKind {
    fn free_into(&self, free: &mut IdSet) {
        self.2.free_into(free);
        self.3.as_slice().free_into(free);
    }
}

impl Free for TableRowKind {
    fn free_into(&self, free: &mut IdSet) {
        self.0.free_into(free);
        self.1.free_into(free);
    }
}

// - Definitions

impl Free for ExternSyntaxDef {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for SyntaxDef {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for SyntaxDefEntry {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for TypDef {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for VarDef {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for ExternRelDef {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for RelDef {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for RuleGroupDef {
    fn free_into(&self, free: &mut IdSet) {
        self.rules.as_slice().free_into(free);
    }
}

impl Free for ExternDecDef {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for BuiltinDecDef {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for TableDecDef {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for FuncDecDef {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for TableDef {
    fn free_into(&self, free: &mut IdSet) {
        self.rows.as_slice().free_into(free);
    }
}

impl Free for FuncDef {
    fn free_into(&self, free: &mut IdSet) {
        self.args.as_slice().free_into(free);
        self.exp.free_into(free);
        self.prems.as_slice().free_into(free);
    }
}

impl Free for DefKind {
    fn free_into(&self, free: &mut IdSet) {
        match self {
            Self::ExternSyntax(definition) => definition.free_into(free),
            Self::Syntax(definition) => definition.free_into(free),
            Self::Typ(definition) => definition.free_into(free),
            Self::Var(definition) => definition.free_into(free),
            Self::ExternRel(definition) => definition.free_into(free),
            Self::Rel(definition) => definition.free_into(free),
            Self::RuleGroup(definition) => definition.free_into(free),
            Self::ExternDec(definition) => definition.free_into(free),
            Self::BuiltinDec(definition) => definition.free_into(free),
            Self::TableDec(definition) => definition.free_into(free),
            Self::FuncDec(definition) => definition.free_into(free),
            Self::TableDef(definition) => definition.free_into(free),
            Self::FuncDef(definition) => definition.free_into(free),
            Self::Sep => {}
        }
    }
}

// - Specifications

impl Free for Spec {
    fn free_into(&self, free: &mut IdSet) {
        self.as_slice().free_into(free);
    }
}
