//! Text rendering for elaboration-language data

use std::fmt::{self, Write};

use crate::lang::{
    traits::print::{Print, Printer},
    xl::num,
};

use super::ast::*;

// - Iterations

impl Print for Iter {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.write(match self {
            Self::Opt => "?",
            Self::List => "*",
        })
    }
}

// - Types

impl Print for Typ {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match self {
            Typ::Plain(plain_typ) => plain_typ.print(printer),
            Typ::Notation(not_typ) => not_typ.print(printer),
        }
    }
}

impl Print for [Typ] {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.separated(self, ", ")
    }
}

// - Plain types

impl Print for PlainTyp {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match &self.node {
            PlainTypKind::Bool => printer.write_str("bool"),
            PlainTypKind::Num(num::Typ::Nat) => printer.write_str("nat"),
            PlainTypKind::Num(num::Typ::Int) => printer.write_str("int"),
            PlainTypKind::Text => printer.write_str("text"),
            PlainTypKind::Var(id, targs) => {
                id.print(printer)?;
                if !targs.is_empty() {
                    printer.write_char('<')?;
                    printer.separated(targs, ", ")?;
                    printer.write_char('>')?;
                }
                Ok(())
            }
            PlainTypKind::Paren(plain_typ) => {
                printer.write_char('(')?;
                plain_typ.print(printer)?;
                printer.write_char(')')
            }
            PlainTypKind::Tuple(plain_typs) => {
                printer.write_char('(')?;
                printer.separated(plain_typs, ", ")?;
                printer.write_char(')')
            }
            PlainTypKind::Iter(plain_typ, iter) => {
                plain_typ.print(printer)?;
                iter.print(printer)
            }
        }
    }
}

// - Notation types

impl Print for NotTyp {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match &self.node {
            NotTypKind::Atom(atom) => atom.print(printer),
            NotTypKind::Seq(typs) => printer.separated(typs, " "),
            NotTypKind::Infix(typ_l, atom, typ_r) => {
                typ_l.print(printer)?;
                printer.write_char(' ')?;
                atom.print(printer)?;
                printer.write_char(' ')?;
                typ_r.print(printer)
            }
            NotTypKind::Brack(atom_l, typ, atom_r) => {
                printer.write_char('`')?;
                atom_l.print(printer)?;
                typ.print(printer)?;
                atom_r.print(printer)
            }
        }
    }
}

impl Print for [NotTyp] {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.separated(self, ", ")
    }
}

// - Defined types

impl Print for DefTyp {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match &self.node {
            DefTypKind::Plain(plain_typ) => plain_typ.print(printer),
            DefTypKind::Struct(typ_fields) => {
                printer.write_char('{')?;
                printer.separated(typ_fields, ", ")?;
                printer.write_char('}')
            }
            DefTypKind::Variant(typ_cases) => {
                printer.write_str("\n   | ")?;
                printer.separated(typ_cases, "\n   | ")
            }
        }
    }
}

impl Print for TypField {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        self.0.print(printer)?;
        printer.write_char(' ')?;
        self.1.print(printer)
    }
}

impl Print for [TypField] {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.separated(self, ", ")
    }
}

impl Print for TypCase {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        self.0.print(printer)
    }
}

impl Print for [TypCase] {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.separated(self, ", ")
    }
}

// - Operators

fn escaped(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'"' => "\\\"".into(),
            b'\\' => "\\\\".into(),
            8 => "\\b".into(),
            9 => "\\t".into(),
            10 => "\\n".into(),
            13 => "\\r".into(),
            32..=126 => char::from(byte).to_string(),
            _ => format!("\\{byte:03}"),
        })
        .collect()
}

impl Print for UnOp {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match self {
            Self::Bool(operator) => operator.print(printer),
            Self::Num(operator) => operator.print(printer),
        }
    }
}

impl Print for BinOp {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match self {
            Self::Bool(operator) => operator.print(printer),
            Self::Num(operator) => operator.print(printer),
        }
    }
}

impl Print for CmpOp {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match self {
            Self::Bool(operator) => operator.print(printer),
            Self::Num(operator) => operator.print(printer),
        }
    }
}

// - Expressions

impl Print for Exp {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match &self.node {
            ExpKind::Bool(value) => write!(printer, "{value}"),
            ExpKind::Num(NumOp::Dec, num::Number::Nat(number)) => write!(printer, "{number}"),
            ExpKind::Num(NumOp::Hex, num::Number::Nat(number)) => {
                write!(
                    printer,
                    "0x{}",
                    number.as_bigint().to_str_radix(16).to_uppercase()
                )
            }
            ExpKind::Num(_, number) => number.print(printer),
            ExpKind::Text(text) => write!(printer, "\"{}\"", escaped(text)),
            ExpKind::Var(id) => printer.write_str(&id.node),
            ExpKind::Un(operator, exp) => {
                operator.print(printer)?;
                exp.print(printer)
            }
            ExpKind::Bin(exp_l, operator, exp_r) => {
                exp_l.print(printer)?;
                printer.write_char(' ')?;
                operator.print(printer)?;
                printer.write_char(' ')?;
                exp_r.print(printer)
            }
            ExpKind::Cmp(exp_l, operator, exp_r) => {
                exp_l.print(printer)?;
                printer.write_char(' ')?;
                operator.print(printer)?;
                printer.write_char(' ')?;
                exp_r.print(printer)
            }
            ExpKind::Arith(exp) => {
                printer.write_str("$(")?;
                exp.print(printer)?;
                printer.write_char(')')
            }
            ExpKind::Eps => printer.write_str("eps"),
            ExpKind::List(exps) => {
                printer.write_char('[')?;
                printer.separated(exps, ", ")?;
                printer.write_char(']')
            }
            ExpKind::Cons(exp_l, exp_r) => {
                exp_l.print(printer)?;
                printer.write_str(" :: ")?;
                exp_r.print(printer)
            }
            ExpKind::Cat(exp_l, exp_r) => {
                exp_l.print(printer)?;
                printer.write_str(" ++ ")?;
                exp_r.print(printer)
            }
            ExpKind::Idx(exp_base, exp_index) => {
                exp_base.print(printer)?;
                printer.write_char('[')?;
                exp_index.print(printer)?;
                printer.write_char(']')
            }
            ExpKind::Slice(exp_base, exp_l, exp_r) => {
                exp_base.print(printer)?;
                printer.write_char('[')?;
                exp_l.print(printer)?;
                printer.write_str(" : ")?;
                exp_r.print(printer)?;
                printer.write_char(']')
            }
            ExpKind::Len(exp) => {
                printer.write_char('|')?;
                exp.print(printer)?;
                printer.write_char('|')
            }
            ExpKind::Mem(exp_l, exp_r) => {
                exp_l.print(printer)?;
                printer.write_str(" <- ")?;
                exp_r.print(printer)
            }
            ExpKind::Str(fields) => {
                printer.write_char('{')?;
                for (index, (atom, exp)) in fields.iter().enumerate() {
                    if index != 0 {
                        printer.write_str(", ")?;
                    }
                    atom.print(printer)?;
                    printer.write_char(' ')?;
                    exp.print(printer)?;
                }
                printer.write_char('}')
            }
            ExpKind::Dot(exp, atom) => {
                exp.print(printer)?;
                printer.write_char('.')?;
                atom.print(printer)
            }
            ExpKind::Upd(exp_base, path, exp_field) => {
                exp_base.print(printer)?;
                printer.write_char('[')?;
                path.print(printer)?;
                printer.write_str(" = ")?;
                exp_field.print(printer)?;
                printer.write_char(']')
            }
            ExpKind::Paren(exp) => {
                printer.write_char('(')?;
                exp.print(printer)?;
                printer.write_char(')')
            }
            ExpKind::Tuple(exps) => {
                printer.write_char('(')?;
                printer.separated(exps, ", ")?;
                printer.write_char(')')
            }
            ExpKind::Call(id, targs, args) => {
                printer.write_char('$')?;
                id.print(printer)?;
                if !targs.is_empty() {
                    printer.write_char('<')?;
                    printer.separated(targs, ", ")?;
                    printer.write_char('>')?;
                }
                args.print(printer)
            }
            ExpKind::Iter(exp, iter) => {
                exp.print(printer)?;
                iter.print(printer)
            }
            ExpKind::Sub(exp, plain_typ) => {
                exp.print(printer)?;
                printer.write_str(" <:")?;
                printer.write_char(' ')?;
                plain_typ.print(printer)
            }
            ExpKind::Atom(atom) => atom.print(printer),
            ExpKind::Seq(exps) => printer.separated(exps, " "),
            ExpKind::Infix(exp_l, atom, exp_r) => {
                exp_l.print(printer)?;
                printer.write_char(' ')?;
                atom.print(printer)?;
                printer.write_char(' ')?;
                exp_r.print(printer)
            }
            ExpKind::Brack(atom_l, exp, atom_r) => {
                printer.write_char('`')?;
                atom_l.print(printer)?;
                exp.print(printer)?;
                atom_r.print(printer)
            }
            ExpKind::Hole(Hole::Num(number)) => write!(printer, "%{number}"),
            ExpKind::Hole(Hole::Next) => printer.write_char('%'),
            ExpKind::Hole(Hole::Rest) => printer.write_str("%%"),
            ExpKind::Hole(Hole::None) => printer.write_str("!%"),
            ExpKind::Fuse(exp_l, exp_r) => {
                exp_l.print(printer)?;
                printer.write_char('#')?;
                exp_r.print(printer)
            }
            ExpKind::Unparen(exp) => {
                printer.write_str("##")?;
                exp.print(printer)
            }
            ExpKind::Latex(text) => write!(printer, "latex({})", escaped(text)),
        }
    }
}

impl Print for [Exp] {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.separated(self, ", ")
    }
}

// - Paths

impl Print for Path {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match &self.node {
            PathKind::Root => Ok(()),
            PathKind::Idx(path, exp_index) => {
                path.print(printer)?;
                printer.write_char('[')?;
                exp_index.print(printer)?;
                printer.write_char(']')
            }
            PathKind::Slice(path, exp_l, exp_r) => {
                path.print(printer)?;
                printer.write_char('[')?;
                exp_l.print(printer)?;
                printer.write_str(" : ")?;
                exp_r.print(printer)?;
                printer.write_char(']')
            }
            PathKind::Dot(path, atom) if matches!(path.node, PathKind::Root) => atom.print(printer),
            PathKind::Dot(path, atom) => {
                path.print(printer)?;
                printer.write_char('.')?;
                atom.print(printer)
            }
        }
    }
}

// - Parameters

impl Print for Param {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match &self.node {
            ParamKind::Exp(plain_typ) => plain_typ.print(printer),
            ParamKind::Def(id, tparams, params, plain_typ) => {
                printer.write_char('$')?;
                id.print(printer)?;
                if !tparams.is_empty() {
                    printer.write_char('<')?;
                    printer.separated(tparams, ", ")?;
                    printer.write_char('>')?;
                }
                params.print(printer)?;
                printer.write_str(" : ")?;
                plain_typ.print(printer)
            }
        }
    }
}

impl Print for [Param] {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        if self.is_empty() {
            Ok(())
        } else {
            printer.write_char('(')?;
            for (index, param) in self.iter().enumerate() {
                if index != 0 {
                    printer.write_str(", ")?;
                }
                param.print(printer)?;
            }
            printer.write_char(')')
        }
    }
}

// - Arguments

impl Print for Arg {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match &self.node {
            ArgKind::Exp(exp) => exp.print(printer),
            ArgKind::Def(id) => {
                printer.write_char('$')?;
                id.print(printer)
            }
        }
    }
}

impl Print for [Arg] {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        if self.is_empty() {
            Ok(())
        } else {
            printer.write_char('(')?;
            for (index, arg) in self.iter().enumerate() {
                if index != 0 {
                    printer.write_str(", ")?;
                }
                arg.print(printer)?;
            }
            printer.write_char(')')
        }
    }
}

// - Premises

impl Print for Prem {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match &self.node {
            PremKind::Var(VarPrem { id, plain_typ }) => {
                id.print(printer)?;
                printer.write_str(" : ")?;
                plain_typ.print(printer)
            }
            PremKind::Rule(RulePrem { id, exp }) => {
                id.print(printer)?;
                printer.write_str(": ")?;
                exp.print(printer)
            }
            PremKind::RuleNot(RuleNotPrem { id, exp }) => {
                id.print(printer)?;
                printer.write_str(":/ ")?;
                exp.print(printer)
            }
            PremKind::If(IfPrem { exp }) => {
                printer.write_str("if ")?;
                exp.print(printer)
            }
            PremKind::Else => printer.write_str("otherwise"),
            PremKind::Iter(IterPrem { prem: inner, iter })
                if matches!(inner.node, PremKind::Iter(_)) =>
            {
                inner.print(printer)?;
                iter.print(printer)
            }
            PremKind::Iter(IterPrem { prem: inner, iter }) => {
                printer.write_char('(')?;
                inner.print(printer)?;
                printer.write_char(')')?;
                iter.print(printer)
            }
            PremKind::Debug(DebugPrem { exp }) => {
                printer.write_str("debug ")?;
                exp.print(printer)
            }
        }
    }
}

impl Print for [Prem] {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        for prem in self {
            printer.write_str("\n -- ")?;
            prem.print(printer)?;
        }
        Ok(())
    }
}

// - Rules

impl Print for Rule {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.write_str("rule ")?;
        self.node.0.print(printer)?;
        if !self.node.1.node.is_empty() {
            printer.write_char('/')?;
            self.node.1.print(printer)?;
        }
        printer.write_str(":\n  ")?;
        self.node.2.print(printer)?;
        self.node.3.print(printer)
    }
}

impl Print for [Rule] {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        for (index, rule) in self.iter().enumerate() {
            if index != 0 {
                printer.write_char('\n')?;
            }
            rule.print(printer)?;
        }
        Ok(())
    }
}

// - Tables

impl Print for TableRow {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        self.node.0.print(printer)?;
        printer.write_str(" => ")?;
        self.node.1.print(printer)
    }
}

impl Print for [TableRow] {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        for (index, row) in self.iter().enumerate() {
            if index != 0 {
                printer.write_str("\n  | ")?;
            }
            row.print(printer)?;
        }
        Ok(())
    }
}

// - Definitions

impl Print for Def {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match &self.node {
            DefKind::ExternSyntax(ExternSyntaxDef { id, .. }) => {
                printer.write_str("extern syntax ")?;
                id.print(printer)
            }
            DefKind::Syntax(SyntaxDef { entries }) => {
                printer.write_str("syntax ")?;
                for (index, SyntaxDefEntry { id, tparams }) in entries.iter().enumerate() {
                    if index != 0 {
                        printer.write_str(", ")?;
                    }
                    id.print(printer)?;
                    if !tparams.is_empty() {
                        printer.write_char('<')?;
                        printer.separated(tparams, ", ")?;
                        printer.write_char('>')?;
                    }
                }
                Ok(())
            }
            DefKind::Typ(TypDef {
                id,
                tparams,
                def_typ,
                ..
            }) => {
                printer.write_str("syntax ")?;
                id.print(printer)?;
                if !tparams.is_empty() {
                    printer.write_char('<')?;
                    printer.separated(tparams, ", ")?;
                    printer.write_char('>')?;
                }
                printer.write_str(" = ")?;
                def_typ.print(printer)
            }
            DefKind::Var(VarDef { id, plain_typ, .. }) => {
                printer.write_str("var ")?;
                id.print(printer)?;
                printer.write_str(" : ")?;
                plain_typ.print(printer)
            }
            DefKind::ExternRel(ExternRelDef { id, not_typ, .. }) => {
                printer.write_str("extern relation ")?;
                id.print(printer)?;
                printer.write_str(": ")?;
                not_typ.print(printer)
            }
            DefKind::Rel(RelDef { id, not_typ, .. }) => {
                printer.write_str("relation ")?;
                id.print(printer)?;
                printer.write_str(": ")?;
                not_typ.print(printer)
            }
            DefKind::RuleGroup(RuleGroupDef {
                relid,
                groupid,
                rules,
            }) => {
                printer.write_str("rulegroup ")?;
                relid.print(printer)?;
                if !groupid.node.is_empty() {
                    printer.write_char('/')?;
                    groupid.print(printer)?;
                }
                printer.write_str(":\n  ")?;
                for (index, rule) in rules.iter().enumerate() {
                    if index != 0 {
                        printer.write_str("\n  ")?;
                    }
                    rule.print(printer)?;
                }
                Ok(())
            }
            DefKind::ExternDec(ExternDecDef {
                id,
                tparams,
                params,
                plain_typ,
                ..
            }) => {
                printer.write_str("extern dec $")?;
                id.print(printer)?;
                if !tparams.is_empty() {
                    printer.write_char('<')?;
                    printer.separated(tparams, ", ")?;
                    printer.write_char('>')?;
                }
                params.print(printer)?;
                printer.write_str(" : ")?;
                plain_typ.print(printer)
            }
            DefKind::BuiltinDec(BuiltinDecDef {
                id,
                tparams,
                params,
                plain_typ,
                ..
            }) => {
                printer.write_str("builtin dec $")?;
                id.print(printer)?;
                if !tparams.is_empty() {
                    printer.write_char('<')?;
                    printer.separated(tparams, ", ")?;
                    printer.write_char('>')?;
                }
                params.print(printer)?;
                printer.write_str(" : ")?;
                plain_typ.print(printer)
            }
            DefKind::TableDec(TableDecDef {
                id,
                params,
                plain_typ,
                ..
            }) => {
                printer.write_str("tbl dec $")?;
                id.print(printer)?;
                params.print(printer)?;
                printer.write_str(" : ")?;
                plain_typ.print(printer)
            }
            DefKind::FuncDec(FuncDecDef {
                id,
                tparams,
                params,
                plain_typ,
                ..
            }) => {
                printer.write_str("dec $")?;
                id.print(printer)?;
                if !tparams.is_empty() {
                    printer.write_char('<')?;
                    printer.separated(tparams, ", ")?;
                    printer.write_char('>')?;
                }
                params.print(printer)?;
                printer.write_str(" : ")?;
                plain_typ.print(printer)
            }
            DefKind::TableDef(TableDef { id, rows }) => {
                printer.write_str("tbl def $")?;
                id.print(printer)?;
                printer.write_str(" =\n  ")?;
                for (index, row) in rows.iter().enumerate() {
                    if index != 0 {
                        printer.write_str("\n  | ")?;
                    }
                    row.node.0.print(printer)?;
                    printer.write_str(" => ")?;
                    row.node.1.print(printer)?;
                }
                Ok(())
            }
            DefKind::FuncDef(FuncDef {
                id,
                tparams,
                args,
                exp,
                prems,
            }) => {
                printer.write_str("def $")?;
                id.print(printer)?;
                if !tparams.is_empty() {
                    printer.write_char('<')?;
                    printer.separated(tparams, ", ")?;
                    printer.write_char('>')?;
                }
                args.print(printer)?;
                printer.write_str(" = ")?;
                exp.print(printer)?;
                prems.print(printer)
            }
            DefKind::Sep => printer.write_str("\n\n"),
        }
    }
}

// - Spec

impl Print for Spec {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        for definition in self {
            definition.print(printer)?;
            printer.write_char('\n')?;
        }
        Ok(())
    }
}
