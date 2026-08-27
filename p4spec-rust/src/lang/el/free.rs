//! Free identifiers in elaboration-language data

use crate::lang::{common::ds::set::IdSet, traits::free::Free};

use super::ast::*;

// == Free identifiers

// - Numbers and text

impl Free for Text {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

// - Identifiers and atoms

impl Free for Id {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

// - Iterators

impl Free for Iter {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

// - Types

impl Free for PlainTyp {
    fn free(&self) -> IdSet {
        self.node.free()
    }
}

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

impl Free for Exp {
    fn free(&self) -> IdSet {
        self.node.free()
    }
}

impl Free for ExpKind {
    fn free(&self) -> IdSet {
        match self {
            Self::Bool(_)
            | Self::Num(_, _)
            | Self::Text(_)
            | Self::Eps
            | Self::Atom(_)
            | Self::Hole(_)
            | Self::Latex(_) => IdSet::new(),
            Self::Var(id) => IdSet::from([id.clone()]),
            Self::Un(_, exp)
            | Self::Arith(exp)
            | Self::Len(exp)
            | Self::Dot(exp, _)
            | Self::Paren(exp)
            | Self::Iter(exp, _)
            | Self::Sub(exp, _)
            | Self::Brack(_, exp, _)
            | Self::Unparen(exp) => exp.free(),
            Self::Bin(exp_l, _, exp_r)
            | Self::Cmp(exp_l, _, exp_r)
            | Self::Cons(exp_l, exp_r)
            | Self::Cat(exp_l, exp_r)
            | Self::Idx(exp_l, exp_r)
            | Self::Mem(exp_l, exp_r)
            | Self::Infix(exp_l, _, exp_r)
            | Self::Fuse(exp_l, exp_r) => exp_l.free().union(exp_r.free()),
            Self::List(exps) | Self::Tuple(exps) | Self::Seq(exps) => exps.as_slice().free(),
            Self::Slice(exp_b, exp_i, exp_n) => {
                exp_b.free().union(exp_i.free()).union(exp_n.free())
            }
            Self::Str(fields) => fields
                .iter()
                .fold(IdSet::new(), |free, (_, exp)| free.union(exp.free())),
            Self::Upd(exp_b, path, exp_f) => exp_b.free().union(path.free()).union(exp_f.free()),
            Self::Call(_, _, args) => args.as_slice().free(),
        }
    }
}

impl Free for Hole {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

// - Paths

impl Free for Path {
    fn free(&self) -> IdSet {
        self.node.free()
    }
}

impl Free for PathKind {
    fn free(&self) -> IdSet {
        match self {
            Self::Root => IdSet::new(),
            Self::Idx(path, exp) => path.free().union(exp.free()),
            Self::Slice(path, exp_i, exp_n) => path.free().union(exp_i.free()).union(exp_n.free()),
            Self::Dot(path, _) => path.free(),
        }
    }
}

// - Arguments

impl Free for Arg {
    fn free(&self) -> IdSet {
        self.node.free()
    }
}

impl Free for ArgKind {
    fn free(&self) -> IdSet {
        match self {
            Self::Exp(exp) => exp.free(),
            Self::Def(_) => IdSet::new(),
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

impl Free for NotTyp {
    fn free(&self) -> IdSet {
        self.node.free()
    }
}

impl Free for NotTypKind {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for DefTyp {
    fn free(&self) -> IdSet {
        self.node.free()
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

impl Free for Param {
    fn free(&self) -> IdSet {
        self.node.free()
    }
}

impl Free for ParamKind {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

// `TParam` aliases `Id` and uses its implementation above.

impl Free for Prem {
    fn free(&self) -> IdSet {
        self.node.free()
    }
}

impl Free for VarPrem {
    fn free(&self) -> IdSet {
        IdSet::from([self.id.clone()])
    }
}

impl Free for RulePrem {
    fn free(&self) -> IdSet {
        self.exp.free()
    }
}

impl Free for RuleNotPrem {
    fn free(&self) -> IdSet {
        self.exp.free()
    }
}

impl Free for IfPrem {
    fn free(&self) -> IdSet {
        self.exp.free()
    }
}

impl Free for IterPrem {
    fn free(&self) -> IdSet {
        self.prem.free()
    }
}

impl Free for DebugPrem {
    fn free(&self) -> IdSet {
        self.exp.free()
    }
}

impl Free for PremKind {
    fn free(&self) -> IdSet {
        match self {
            Self::Var(prem) => prem.free(),
            Self::Rule(prem) => prem.free(),
            Self::RuleNot(prem) => prem.free(),
            Self::If(prem) => prem.free(),
            Self::Else => IdSet::new(),
            Self::Iter(prem) => prem.free(),
            Self::Debug(prem) => prem.free(),
        }
    }
}

// - Rules and tables

impl Free for Rule {
    fn free(&self) -> IdSet {
        self.node.free()
    }
}

impl Free for RuleKind {
    fn free(&self) -> IdSet {
        self.2.free().union(self.3.as_slice().free())
    }
}

impl Free for TableRow {
    fn free(&self) -> IdSet {
        self.node.free()
    }
}

impl Free for TableRowKind {
    fn free(&self) -> IdSet {
        self.0.free().union(self.1.free())
    }
}

// - Definitions

impl Free for Def {
    fn free(&self) -> IdSet {
        self.node.free()
    }
}

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
    fn free(&self) -> IdSet {
        self.rules.as_slice().free()
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
    fn free(&self) -> IdSet {
        self.rows.as_slice().free()
    }
}

impl Free for FuncDef {
    fn free(&self) -> IdSet {
        self.args
            .as_slice()
            .free()
            .union(self.exp.free())
            .union(self.prems.as_slice().free())
    }
}

impl Free for DefKind {
    fn free(&self) -> IdSet {
        match self {
            Self::ExternSyntax(definition) => definition.free(),
            Self::Syntax(definition) => definition.free(),
            Self::Typ(definition) => definition.free(),
            Self::Var(definition) => definition.free(),
            Self::ExternRel(definition) => definition.free(),
            Self::Rel(definition) => definition.free(),
            Self::RuleGroup(definition) => definition.free(),
            Self::ExternDec(definition) => definition.free(),
            Self::BuiltinDec(definition) => definition.free(),
            Self::TableDec(definition) => definition.free(),
            Self::FuncDec(definition) => definition.free(),
            Self::TableDef(definition) => definition.free(),
            Self::FuncDef(definition) => definition.free(),
            Self::Sep => IdSet::new(),
        }
    }
}

impl Free for Spec {
    fn free(&self) -> IdSet {
        self.as_slice().free()
    }
}
