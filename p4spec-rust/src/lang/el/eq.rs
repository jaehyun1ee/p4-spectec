//! Syntax equality for elaboration-language data
//!
//! Ignores source regions while comparing parsed syntax and hints

use crate::lang::traits::eq::SyntaxEq;

use super::ast::*;

// == Syntax equality

impl SyntaxEq for Id {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.node == other.node
    }
}

impl SyntaxEq for [Id] {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .zip(other)
                .all(|(id_l, id_r)| id_l.syntax_eq(id_r))
    }
}

impl SyntaxEq for Iter {
    fn syntax_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl SyntaxEq for PlainTyp {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.node.syntax_eq(&other.node)
    }
}

impl SyntaxEq for PlainTypKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PlainTypKind::Bool, PlainTypKind::Bool) | (PlainTypKind::Text, PlainTypKind::Text) => {
                true
            }
            (PlainTypKind::Num(typ_l), PlainTypKind::Num(typ_r)) => typ_l == typ_r,
            (PlainTypKind::Var(id_l, targs_l), PlainTypKind::Var(id_r, targs_r)) => {
                id_l.syntax_eq(id_r) && targs_l.syntax_eq(targs_r)
            }
            (PlainTypKind::Paren(typ_l), PlainTypKind::Paren(typ_r)) => typ_l.syntax_eq(typ_r),
            (PlainTypKind::Tuple(typs_l), PlainTypKind::Tuple(typs_r)) => typs_l.syntax_eq(typs_r),
            (PlainTypKind::Iter(typ_l, iter_l), PlainTypKind::Iter(typ_r, iter_r)) => {
                typ_l.syntax_eq(typ_r) && iter_l.syntax_eq(iter_r)
            }
            _ => false,
        }
    }
}

impl SyntaxEq for [PlainTyp] {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .zip(other)
                .all(|(typ_l, typ_r)| typ_l.syntax_eq(typ_r))
    }
}

impl SyntaxEq for NumOp {
    fn syntax_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl SyntaxEq for UnOp {
    fn syntax_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl SyntaxEq for BinOp {
    fn syntax_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl SyntaxEq for CmpOp {
    fn syntax_eq(&self, other: &Self) -> bool {
        self == other
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
            (ExpKind::Num(op_l, num_l), ExpKind::Num(op_r, num_r)) => {
                op_l.syntax_eq(op_r) && num_l == num_r
            }
            (ExpKind::Text(text_l), ExpKind::Text(text_r)) => text_l == text_r,
            (ExpKind::Var(id_l), ExpKind::Var(id_r)) => id_l.syntax_eq(id_r),
            (ExpKind::Un(op_l, exp_l), ExpKind::Un(op_r, exp_r)) => {
                op_l.syntax_eq(op_r) && exp_l.syntax_eq(exp_r)
            }
            (ExpKind::Bin(exp_l_l, op_l, exp_r_l), ExpKind::Bin(exp_l_r, op_r, exp_r_r)) => {
                exp_l_l.syntax_eq(exp_l_r) && op_l.syntax_eq(op_r) && exp_r_l.syntax_eq(exp_r_r)
            }
            (ExpKind::Cmp(exp_l_l, op_l, exp_r_l), ExpKind::Cmp(exp_l_r, op_r, exp_r_r)) => {
                exp_l_l.syntax_eq(exp_l_r) && op_l.syntax_eq(op_r) && exp_r_l.syntax_eq(exp_r_r)
            }
            (ExpKind::Arith(exp_l), ExpKind::Arith(exp_r))
            | (ExpKind::Paren(exp_l), ExpKind::Paren(exp_r))
            | (ExpKind::Unparen(exp_l), ExpKind::Unparen(exp_r))
            | (ExpKind::Len(exp_l), ExpKind::Len(exp_r)) => exp_l.syntax_eq(exp_r),
            (ExpKind::Eps, ExpKind::Eps) => true,
            (ExpKind::List(exps_l), ExpKind::List(exps_r))
            | (ExpKind::Tuple(exps_l), ExpKind::Tuple(exps_r))
            | (ExpKind::Seq(exps_l), ExpKind::Seq(exps_r)) => exps_l.syntax_eq(exps_r),
            (ExpKind::Cons(exp_l_l, exp_r_l), ExpKind::Cons(exp_l_r, exp_r_r))
            | (ExpKind::Cat(exp_l_l, exp_r_l), ExpKind::Cat(exp_l_r, exp_r_r))
            | (ExpKind::Mem(exp_l_l, exp_r_l), ExpKind::Mem(exp_l_r, exp_r_r))
            | (ExpKind::Fuse(exp_l_l, exp_r_l), ExpKind::Fuse(exp_l_r, exp_r_r)) => {
                exp_l_l.syntax_eq(exp_l_r) && exp_r_l.syntax_eq(exp_r_r)
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
            (ExpKind::Str(fields_l), ExpKind::Str(fields_r)) => {
                fields_l.len() == fields_r.len()
                    && fields_l
                        .iter()
                        .zip(fields_r)
                        .all(|((atom_l, exp_l), (atom_r, exp_r))| {
                            atom_l.syntax_eq(atom_r) && exp_l.syntax_eq(exp_r)
                        })
            }
            (ExpKind::Dot(exp_l, atom_l), ExpKind::Dot(exp_r, atom_r)) => {
                exp_l.syntax_eq(exp_r) && atom_l.syntax_eq(atom_r)
            }
            (ExpKind::Upd(exp_b_l, path_l, exp_f_l), ExpKind::Upd(exp_b_r, path_r, exp_f_r)) => {
                exp_b_l.syntax_eq(exp_b_r) && path_l.syntax_eq(path_r) && exp_f_l.syntax_eq(exp_f_r)
            }
            (ExpKind::Call(id_l, targs_l, args_l), ExpKind::Call(id_r, targs_r, args_r)) => {
                id_l.syntax_eq(id_r) && targs_l.syntax_eq(targs_r) && args_l.syntax_eq(args_r)
            }
            (ExpKind::Iter(exp_l, iter_l), ExpKind::Iter(exp_r, iter_r)) => {
                exp_l.syntax_eq(exp_r) && iter_l.syntax_eq(iter_r)
            }
            (ExpKind::Sub(exp_l, typ_l), ExpKind::Sub(exp_r, typ_r)) => {
                exp_l.syntax_eq(exp_r) && typ_l.syntax_eq(typ_r)
            }
            (ExpKind::Atom(atom_l), ExpKind::Atom(atom_r)) => atom_l.syntax_eq(atom_r),
            (
                ExpKind::Infix(exp_l_l, atom_l, exp_r_l),
                ExpKind::Infix(exp_l_r, atom_r, exp_r_r),
            ) => {
                exp_l_l.syntax_eq(exp_l_r) && atom_l.syntax_eq(atom_r) && exp_r_l.syntax_eq(exp_r_r)
            }
            (
                ExpKind::Brack(atom_l_l, exp_l, atom_r_l),
                ExpKind::Brack(atom_l_r, exp_r, atom_r_r),
            ) => {
                atom_l_l.syntax_eq(atom_l_r)
                    && exp_l.syntax_eq(exp_r)
                    && atom_r_l.syntax_eq(atom_r_r)
            }
            (ExpKind::Hole(hole_l), ExpKind::Hole(hole_r)) => hole_l.syntax_eq(hole_r),
            (ExpKind::Latex(text_l), ExpKind::Latex(text_r)) => text_l == text_r,
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

impl SyntaxEq for Hole {
    fn syntax_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl SyntaxEq for Path {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.node.syntax_eq(&other.node)
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

impl SyntaxEq for Hint {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.0.syntax_eq(&other.0) && self.1.syntax_eq(&other.1)
    }
}

impl SyntaxEq for [Hint] {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .zip(other)
                .all(|(hint_l, hint_r)| hint_l.syntax_eq(hint_r))
    }
}

impl SyntaxEq for Typ {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Typ::Plain(typ_l), Typ::Plain(typ_r)) => typ_l.syntax_eq(typ_r),
            (Typ::Notation(typ_l), Typ::Notation(typ_r)) => typ_l.syntax_eq(typ_r),
            _ => false,
        }
    }
}

impl SyntaxEq for [Typ] {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .zip(other)
                .all(|(typ_l, typ_r)| typ_l.syntax_eq(typ_r))
    }
}

impl SyntaxEq for NotTyp {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.node.syntax_eq(&other.node)
    }
}

impl SyntaxEq for NotTypKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (NotTypKind::Atom(atom_l), NotTypKind::Atom(atom_r)) => atom_l.syntax_eq(atom_r),
            (NotTypKind::Seq(typs_l), NotTypKind::Seq(typs_r)) => typs_l.syntax_eq(typs_r),
            (
                NotTypKind::Infix(typ_l_l, atom_l, typ_r_l),
                NotTypKind::Infix(typ_l_r, atom_r, typ_r_r),
            ) => {
                typ_l_l.syntax_eq(typ_l_r) && atom_l.syntax_eq(atom_r) && typ_r_l.syntax_eq(typ_r_r)
            }
            (
                NotTypKind::Brack(atom_l_l, typ_l, atom_r_l),
                NotTypKind::Brack(atom_l_r, typ_r, atom_r_r),
            ) => {
                atom_l_l.syntax_eq(atom_l_r)
                    && typ_l.syntax_eq(typ_r)
                    && atom_r_l.syntax_eq(atom_r_r)
            }
            _ => false,
        }
    }
}

impl SyntaxEq for DefTyp {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.node.syntax_eq(&other.node)
    }
}

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
        self.0.syntax_eq(&other.0) && self.1.syntax_eq(&other.1) && self.2.syntax_eq(&other.2)
    }
}

impl SyntaxEq for [TypField] {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .zip(other)
                .all(|(field_l, field_r)| field_l.syntax_eq(field_r))
    }
}

impl SyntaxEq for TypCase {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.0.syntax_eq(&other.0) && self.1.syntax_eq(&other.1)
    }
}

impl SyntaxEq for [TypCase] {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .zip(other)
                .all(|(case_l, case_r)| case_l.syntax_eq(case_r))
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

impl SyntaxEq for [Param] {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .zip(other)
                .all(|(param_l, param_r)| param_l.syntax_eq(param_r))
    }
}

impl SyntaxEq for VarPrem {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id) && self.plain_typ.syntax_eq(&other.plain_typ)
    }
}

impl SyntaxEq for RulePrem {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id) && self.exp.syntax_eq(&other.exp)
    }
}

impl SyntaxEq for RuleNotPrem {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id) && self.exp.syntax_eq(&other.exp)
    }
}

impl SyntaxEq for IfPrem {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.exp.syntax_eq(&other.exp)
    }
}

impl SyntaxEq for IterPrem {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.prem.syntax_eq(&other.prem) && self.iter.syntax_eq(&other.iter)
    }
}

impl SyntaxEq for DebugPrem {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.exp.syntax_eq(&other.exp)
    }
}

impl SyntaxEq for Prem {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.node.syntax_eq(&other.node)
    }
}

impl SyntaxEq for PremKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PremKind::Var(prem_l), PremKind::Var(prem_r)) => prem_l.syntax_eq(prem_r),
            (PremKind::Rule(prem_l), PremKind::Rule(prem_r)) => prem_l.syntax_eq(prem_r),
            (PremKind::RuleNot(prem_l), PremKind::RuleNot(prem_r)) => prem_l.syntax_eq(prem_r),
            (PremKind::If(prem_l), PremKind::If(prem_r)) => prem_l.syntax_eq(prem_r),
            (PremKind::Else, PremKind::Else) => true,
            (PremKind::Iter(prem_l), PremKind::Iter(prem_r)) => prem_l.syntax_eq(prem_r),
            (PremKind::Debug(prem_l), PremKind::Debug(prem_r)) => prem_l.syntax_eq(prem_r),
            _ => false,
        }
    }
}

impl SyntaxEq for [Prem] {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .zip(other)
                .all(|(prem_l, prem_r)| prem_l.syntax_eq(prem_r))
    }
}

impl SyntaxEq for RuleKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.0.syntax_eq(&other.0)
            && self.1.syntax_eq(&other.1)
            && self.2.syntax_eq(&other.2)
            && self.3.syntax_eq(&other.3)
    }
}

impl SyntaxEq for Rule {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.node.syntax_eq(&other.node)
    }
}

impl SyntaxEq for [Rule] {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .zip(other)
                .all(|(rule_l, rule_r)| rule_l.syntax_eq(rule_r))
    }
}

impl SyntaxEq for TableRowKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.0.syntax_eq(&other.0) && self.1.syntax_eq(&other.1)
    }
}

impl SyntaxEq for TableRow {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.node.syntax_eq(&other.node)
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

impl SyntaxEq for ExternSyntaxDef {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id) && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for SyntaxDefEntry {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id) && self.tparams.syntax_eq(&other.tparams)
    }
}

impl SyntaxEq for [SyntaxDefEntry] {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .zip(other)
                .all(|(entry_l, entry_r)| entry_l.syntax_eq(entry_r))
    }
}

impl SyntaxEq for SyntaxDef {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.entries.syntax_eq(&other.entries)
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
            && self.plain_typ.syntax_eq(&other.plain_typ)
            && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for ExternRelDef {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.not_typ.syntax_eq(&other.not_typ)
            && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for RelDef {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.not_typ.syntax_eq(&other.not_typ)
            && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for RuleGroupDef {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.relid.syntax_eq(&other.relid)
            && self.groupid.syntax_eq(&other.groupid)
            && self.rules.syntax_eq(&other.rules)
    }
}

impl SyntaxEq for ExternDecDef {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.tparams.syntax_eq(&other.tparams)
            && self.params.syntax_eq(&other.params)
            && self.plain_typ.syntax_eq(&other.plain_typ)
            && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for BuiltinDecDef {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.tparams.syntax_eq(&other.tparams)
            && self.params.syntax_eq(&other.params)
            && self.plain_typ.syntax_eq(&other.plain_typ)
            && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for TableDecDef {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.params.syntax_eq(&other.params)
            && self.plain_typ.syntax_eq(&other.plain_typ)
            && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for FuncDecDef {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.tparams.syntax_eq(&other.tparams)
            && self.params.syntax_eq(&other.params)
            && self.plain_typ.syntax_eq(&other.plain_typ)
            && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for TableDef {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id) && self.rows.syntax_eq(&other.rows)
    }
}

impl SyntaxEq for FuncDef {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.tparams.syntax_eq(&other.tparams)
            && self.args.syntax_eq(&other.args)
            && self.exp.syntax_eq(&other.exp)
            && self.prems.syntax_eq(&other.prems)
    }
}

impl SyntaxEq for DefKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (DefKind::ExternSyntax(def_l), DefKind::ExternSyntax(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::Syntax(def_l), DefKind::Syntax(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::Typ(def_l), DefKind::Typ(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::Var(def_l), DefKind::Var(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::ExternRel(def_l), DefKind::ExternRel(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::Rel(def_l), DefKind::Rel(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::RuleGroup(def_l), DefKind::RuleGroup(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::ExternDec(def_l), DefKind::ExternDec(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::BuiltinDec(def_l), DefKind::BuiltinDec(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::TableDec(def_l), DefKind::TableDec(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::FuncDec(def_l), DefKind::FuncDec(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::TableDef(def_l), DefKind::TableDef(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::FuncDef(def_l), DefKind::FuncDef(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::Sep, DefKind::Sep) => true,
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
