//! Text rendering for intermediate-language data

use std::fmt::{self, Write};

use crate::lang::{
    traits::print::{Print, Printer},
    xl::num,
};

use super::ast::*;

// == Printing

// - Helpers

fn indent(level: usize) -> String {
    "  ".repeat(level)
}
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

// - Variables

impl Print for Var {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        self.id.print(printer)?;
        for iter in &self.iters {
            iter.print(printer)?;
        }
        Ok(())
    }
}

// - Types

impl Print for Typ {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match &self.node {
            TypKind::Bool => printer.write_str("bool"),
            TypKind::Num(num::Typ::Nat) => printer.write_str("nat"),
            TypKind::Num(num::Typ::Int) => printer.write_str("int"),
            TypKind::Text => printer.write_str("text"),
            TypKind::Var(id, targs) => {
                id.print(printer)?;
                if !targs.is_empty() {
                    printer.write_char('<')?;
                    printer.separated(targs, ", ")?;
                    printer.write_char('>')?;
                }
                Ok(())
            }
            TypKind::Tuple(typs) => {
                printer.write_char('(')?;
                printer.separated(typs, ", ")?;
                printer.write_char(')')
            }
            TypKind::Iter(typ, iter) => {
                typ.print(printer)?;
                iter.print(printer)
            }
            TypKind::Func(func_typ) => {
                if !func_typ.tparams.is_empty() {
                    printer.write_char('<')?;
                    printer.separated(&func_typ.tparams, ", ")?;
                    printer.write_char('>')?;
                }
                printer.write_char('(')?;
                printer.separated(&func_typ.typs_params, ", ")?;
                printer.write_str(") : ")?;
                func_typ.typ_ret.print(printer)
            }
        }
    }
}

impl Print for NotTyp {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        self.node
            .print_with(printer, |typ, printer| typ.print(printer))
    }
}

impl Print for DefTyp {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match &self.node {
            DefTypKind::Plain(typ) => typ.print(printer),
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

impl Print for TypOrigin {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.write_str("(from ")?;
        self.node.0.print(printer)?;
        if !self.node.1.is_empty() {
            printer.write_char('<')?;
            printer.separated(&self.node.1, ", ")?;
            printer.write_char('>')?;
        }
        printer.write_char(')')
    }
}

impl Print for TypCase {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        let (not_typ, typ_origin, hints) = self;
        not_typ.print(printer)?;
        printer.write_char(' ')?;
        typ_origin.print(printer)?;
        printer.write_char(' ')?;
        hints.print(printer)
    }
}

impl Print for [TypCase] {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.separated(self, ", ")
    }
}

// - Values

fn write_value_with(
    output: &mut Printer<'_>,
    value: &Value,
    short: bool,
    level: usize,
) -> fmt::Result {
    match &value.node {
        ValueKind::Bool(value) => write!(output, "{value}"),
        ValueKind::Num(value) => value.print(output),
        ValueKind::Text(text) => output.write_str(&escaped(text)),
        ValueKind::Struct(fields) if fields.is_empty() => output.write_str("{}"),
        ValueKind::Struct(fields) if short => write!(output, "{{ .../{} }}", fields.len()),
        ValueKind::Struct(fields) => {
            output.write_str("{\n")?;
            for (index, (atom, value)) in fields.iter().enumerate() {
                if index != 0 {
                    output.write_str(";\n")?;
                }
                output.write_str(&indent(level + 1))?;
                atom.print(output)?;
                output.write_char(' ')?;
                write_value_with(output, value, short, level + 1)?;
            }
            output.write_char('\n')?;
            output.write_str(&indent(level))?;
            output.write_char('}')
        }
        ValueKind::Case(case) if short => case.to_mixop().print(output),
        ValueKind::Case(case) => write_notval_with(output, case, level),
        ValueKind::Tuple(values) => {
            output.write_char('(')?;
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.write_str(", ")?;
                }
                write_value_with(output, value, short, level + 1)?;
            }
            output.write_char(')')
        }
        ValueKind::Opt(Some(value)) => {
            output.write_str("Some(")?;
            write_value_with(output, value, short, level + 1)?;
            output.write_char(')')
        }
        ValueKind::Opt(None) => output.write_str("None"),
        ValueKind::List(values) if values.is_empty() => output.write_str("[]"),
        ValueKind::List(values) if short => write!(output, "[ .../{} ]", values.len()),
        ValueKind::List(values) => {
            output.write_str("[\n")?;
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.write_str(",\n")?;
                }
                output.write_str(&indent(level + 1))?;
                write_value_with(output, value, short, level + 1)?;
            }
            output.write_char('\n')?;
            output.write_str(&indent(level))?;
            output.write_char(']')
        }
        ValueKind::Func(id) => {
            output.write_char('$')?;
            id.print(output)
        }
        ValueKind::Extern(_) => output.write_str("extern"),
    }
}

fn write_notval_with(output: &mut Printer<'_>, not_val: &ValueCase, level: usize) -> fmt::Result {
    not_val.print_with(output, |value, output| {
        write_value_with(output, value, false, level + 1)
    })
}

impl Print for Value {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        write_value_with(printer, self, false, 0)
    }
}

impl Print for ValueCase {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        write_notval_with(printer, self, 0)
    }
}

// - Expressions

impl Print for Exp {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match &self.node {
            ExpKind::Bool(value) => write!(printer, "{value}"),
            ExpKind::Num(value) => value.print(printer),
            ExpKind::Text(text) => write!(printer, "\"{}\"", escaped(text)),
            ExpKind::Var(id) => printer.write_str(&id.node),
            ExpKind::Un(op, _, exp) => {
                op.print(printer)?;
                exp.print(printer)
            }
            ExpKind::Bin(op, _, exp_l, exp_r) => {
                printer.write_char('(')?;
                exp_l.print(printer)?;
                printer.write_char(' ')?;
                op.print(printer)?;
                printer.write_char(' ')?;
                exp_r.print(printer)?;
                printer.write_char(')')
            }
            ExpKind::Cmp(op, _, exp_l, exp_r) => {
                printer.write_char('(')?;
                exp_l.print(printer)?;
                printer.write_char(' ')?;
                op.print(printer)?;
                printer.write_char(' ')?;
                exp_r.print(printer)?;
                printer.write_char(')')
            }
            ExpKind::UpCast(typ, exp) | ExpKind::DownCast(typ, exp) => {
                exp.print(printer)?;
                printer.write_str(" as ")?;
                typ.print(printer)
            }
            ExpKind::Sub(exp, typ, _) => {
                exp.print(printer)?;
                printer.write_str(" <: ")?;
                typ.print(printer)
            }
            ExpKind::Match(exp, pattern) => {
                exp.print(printer)?;
                printer.write_str(" matches ")?;
                pattern.print(printer)
            }
            ExpKind::Tuple(exps) => {
                printer.write_char('(')?;
                printer.separated(exps, ", ")?;
                printer.write_char(')')
            }
            ExpKind::Case(not_exp) => not_exp.print(printer),
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
            ExpKind::Opt(exp) => {
                printer.write_str("?(")?;
                if let Some(exp) = exp {
                    exp.print(printer)?;
                }
                printer.write_char(')')
            }
            ExpKind::List(exps) => {
                printer.write_char('[')?;
                printer.separated(exps, ", ")?;
                printer.write_char(']')
            }
            ExpKind::Cons(head, tail) => {
                head.print(printer)?;
                printer.write_str(" :: ")?;
                tail.print(printer)
            }
            ExpKind::Cat(exp_l, exp_r) => {
                exp_l.print(printer)?;
                printer.write_str(" ++ ")?;
                exp_r.print(printer)
            }
            ExpKind::Mem(exp_e, exp_s) => {
                exp_e.print(printer)?;
                printer.write_str(" <- ")?;
                exp_s.print(printer)
            }
            ExpKind::Len(exp) => {
                printer.write_char('|')?;
                exp.print(printer)?;
                printer.write_char('|')
            }
            ExpKind::Dot(exp, atom) => {
                exp.print(printer)?;
                printer.write_char('.')?;
                atom.print(printer)
            }
            ExpKind::Idx(exp_b, exp_i) => {
                exp_b.print(printer)?;
                printer.write_char('[')?;
                exp_i.print(printer)?;
                printer.write_char(']')
            }
            ExpKind::Slice(exp_b, exp_i, exp_n) => {
                exp_b.print(printer)?;
                printer.write_char('[')?;
                exp_i.print(printer)?;
                printer.write_str(" : ")?;
                exp_n.print(printer)?;
                printer.write_char(']')
            }
            ExpKind::Upd(exp_b, path, exp_f) => {
                exp_b.print(printer)?;
                printer.write_char('[')?;
                path.print(printer)?;
                printer.write_str(" = ")?;
                exp_f.print(printer)?;
                printer.write_char(']')
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
            ExpKind::Iter(exp, iter_exp) => {
                exp.print(printer)?;
                iter_exp.print(printer)
            }
        }
    }
}

impl Print for [Exp] {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.separated(self, ", ")
    }
}

impl Print for NotExp {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        self.print_with(printer, |exp, printer| exp.print(printer))
    }
}

impl Print for ExpIter {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        self.0.print(printer)?;
        printer.write_char('{')?;
        for (index, var) in self.1.iter().enumerate() {
            if index != 0 {
                printer.write_str(", ")?;
            }
            let mut var_iter = var.clone();
            var_iter.iters.push(self.0);
            var.print(printer)?;
            printer.write_str(" <- ")?;
            var_iter.print(printer)?;
        }
        printer.write_char('}')
    }
}

impl Print for [ExpIter] {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        for iter_exp in self {
            iter_exp.print(printer)?;
        }
        Ok(())
    }
}

// - Patterns

impl Print for Pattern {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match self {
            Pattern::Case(mixop) => mixop.print(printer),
            Pattern::List(ListPattern::Cons) => printer.write_str("_ :: _"),
            Pattern::List(ListPattern::Fixed(length)) => write!(printer, "[ _/{length} ]"),
            Pattern::List(ListPattern::Nil) => printer.write_str("[]"),
            Pattern::Opt(OptPattern::Some) => printer.write_str("(_)"),
            Pattern::Opt(OptPattern::None) => printer.write_str("()"),
        }
    }
}

// - Paths

impl Print for Path {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match &self.node {
            PathKind::Root => Ok(()),
            PathKind::Idx(path, exp_i) => {
                path.print(printer)?;
                printer.write_char('[')?;
                exp_i.print(printer)?;
                printer.write_char(']')
            }
            PathKind::Slice(path, exp_i, exp_n) => {
                path.print(printer)?;
                printer.write_char('[')?;
                exp_i.print(printer)?;
                printer.write_str(" : ")?;
                exp_n.print(printer)?;
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
            ParamKind::Exp(typ) => typ.print(printer),
            ParamKind::Def(id, tparams, params, typ) => {
                printer.write_char('$')?;
                id.print(printer)?;
                if !tparams.is_empty() {
                    printer.write_char('<')?;
                    printer.separated(tparams, ", ")?;
                    printer.write_char('>')?;
                }
                params.print(printer)?;
                printer.write_str(" : ")?;
                typ.print(printer)
            }
        }
    }
}

impl Print for [Param] {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        if self.is_empty() {
            return Ok(());
        }
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
            return Ok(());
        }
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

// - Premises

fn write_prems_with(output: &mut Printer<'_>, level: usize, prems: &[Prem]) -> fmt::Result {
    for prem in prems {
        write!(output, "\n{}-- ", indent(level))?;
        prem.print(output)?;
    }
    Ok(())
}

impl Print for Prem {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match &self.node {
            PremKind::Rule(RulePrem { id, not_exp, .. }) => {
                id.print(printer)?;
                printer.write_str(": ")?;
                not_exp.print(printer)
            }
            PremKind::If(IfPrem { exp }) => {
                printer.write_str("if ")?;
                exp.print(printer)
            }
            PremKind::IfHold(IfHoldPrem { id, not_exp }) => {
                printer.write_str("if ")?;
                id.print(printer)?;
                printer.write_str(": ")?;
                not_exp.print(printer)?;
                printer.write_str(" holds")
            }
            PremKind::IfNotHold(IfNotHoldPrem { id, not_exp }) => {
                printer.write_str("if ")?;
                id.print(printer)?;
                printer.write_str(": ")?;
                not_exp.print(printer)?;
                printer.write_str(" does not hold")
            }
            PremKind::Iter(IterPrem {
                prem: inner,
                prem_iter,
            }) if matches!(inner.node, PremKind::Iter(_)) => {
                inner.print(printer)?;
                prem_iter.print(printer)
            }
            PremKind::Iter(IterPrem {
                prem: inner,
                prem_iter,
            }) => {
                printer.write_char('(')?;
                inner.print(printer)?;
                printer.write_char(')')?;
                prem_iter.print(printer)
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
        write_prems_with(printer, 0, self)
    }
}

impl Print for PremIter {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        self.iter.print(printer)?;
        printer.write_char('{')?;
        let vars = self
            .vars_bound
            .iter()
            .map(|var| (var, "<-"))
            .chain(self.vars_bind.iter().map(|var| (var, "->")));
        for (index, (var, arrow)) in vars.enumerate() {
            if index != 0 {
                printer.write_str(", ")?;
            }
            let mut var_iter = var.clone();
            var_iter.iters.push(self.iter);
            var.print(printer)?;
            write!(printer, " {arrow} ")?;
            var_iter.print(printer)?;
        }
        printer.write_char('}')
    }
}

impl Print for [PremIter] {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        for prem_iter in self {
            prem_iter.print(printer)?;
        }
        Ok(())
    }
}

// - Rules

impl Print for Rule {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.write_str("rule ")?;
        self.node.id.print(printer)?;
        printer.write_str(": ")?;
        self.node.not_exp.print(printer)?;
        write_prems_with(printer, 2, &self.node.prems)
    }
}

impl Print for [Rule] {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        for rule in self {
            printer.write_str("\n\n  ")?;
            rule.print(printer)?;
        }
        Ok(())
    }
}

impl Print for RuleGroup {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.write_str("  rulegroup ")?;
        self.node.0.print(printer)?;
        for rule in &self.node.1 {
            printer.write_str("\n\n    ")?;
            rule.print(printer)?;
        }
        Ok(())
    }
}

impl Print for [RuleGroup] {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        for (index, rule_group) in self.iter().enumerate() {
            if index != 0 {
                printer.write_str("\n\n")?;
            }
            rule_group.print(printer)?;
        }
        Ok(())
    }
}

impl Print for ElseGroup {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.write_str("  rulegroup ")?;
        self.node.0.print(printer)?;
        printer.write_str("\n\n    ")?;
        self.node.1.print(printer)
    }
}

impl Print for Option<ElseGroup> {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        if let Some(else_group) = self {
            printer.write_str("\n\n  elsegroup\n\n")?;
            else_group.print(printer)?;
        }
        Ok(())
    }
}

// - Clauses

impl Print for Clause {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        self.node.args.print(printer)?;
        printer.write_str(" = ")?;
        self.node.expression.print(printer)?;
        write_prems_with(printer, 1, &self.node.premises)
    }
}

// - Table rows

impl Print for TableRow {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.write_str("\n    ")?;
        self.node.0.print(printer)?;
        printer.write_str(" -> ")?;
        self.node.1.print(printer)
    }
}

impl Print for [TableRow] {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        for (index, table_row) in self.iter().enumerate() {
            write!(printer, "\n  row {index} :")?;
            table_row.print(printer)?;
        }
        Ok(())
    }
}

// - Hints

impl Print for Hint {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        write!(printer, " hint({} ", self.0.node)?;
        self.1.print(printer)?;
        printer.write_char(')')
    }
}

impl Print for [Hint] {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        for hint in self {
            hint.print(printer)?;
        }
        Ok(())
    }
}

// - Definitions

impl Print for Def {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match &self.node {
            DefKind::ExternTyp(ExternTyp { id, .. }) => {
                printer.write_str("extern syntax ")?;
                id.print(printer)
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
            DefKind::Var(VarDef { id, typ, .. }) => {
                printer.write_str("var ")?;
                id.print(printer)?;
                printer.write_str(" : ")?;
                typ.print(printer)
            }
            DefKind::ExternRel(ExternRel { id, not_typ, .. }) => {
                printer.write_str("extern relation ")?;
                id.print(printer)?;
                printer.write_str(": ")?;
                not_typ.print(printer)
            }
            DefKind::Rel(Rel {
                id,
                not_typ,
                rule_groups,
                else_group,
                ..
            }) => {
                printer.write_str("relation ")?;
                id.print(printer)?;
                printer.write_str(": ")?;
                not_typ.print(printer)?;
                printer.write_str("\n\n")?;
                rule_groups.print(printer)?;
                else_group.print(printer)
            }
            DefKind::ExternDec(ExternDec {
                id,
                tparams,
                params,
                typ,
                ..
            }) => {
                printer.write_str("extern def $")?;
                id.print(printer)?;
                if !tparams.is_empty() {
                    printer.write_char('<')?;
                    printer.separated(tparams, ", ")?;
                    printer.write_char('>')?;
                }
                params.print(printer)?;
                printer.write_str(" : ")?;
                typ.print(printer)
            }
            DefKind::BuiltinDec(BuiltinDec {
                id,
                tparams,
                params,
                typ,
                ..
            }) => {
                printer.write_str("builtin def $")?;
                id.print(printer)?;
                if !tparams.is_empty() {
                    printer.write_char('<')?;
                    printer.separated(tparams, ", ")?;
                    printer.write_char('>')?;
                }
                params.print(printer)?;
                printer.write_str(" : ")?;
                typ.print(printer)
            }
            DefKind::TableDec(TableDec {
                id,
                params,
                typ,
                rows,
                ..
            }) => {
                printer.write_str("tbl def $")?;
                id.print(printer)?;
                params.print(printer)?;
                printer.write_str(" : ")?;
                typ.print(printer)?;
                printer.write_str(" =")?;
                rows.print(printer)
            }
            DefKind::FuncDec(FuncDec {
                id,
                tparams,
                params,
                typ,
                clauses,
                else_clause,
                ..
            }) => {
                printer.write_str("def $")?;
                id.print(printer)?;
                if !tparams.is_empty() {
                    printer.write_char('<')?;
                    printer.separated(tparams, ", ")?;
                    printer.write_char('>')?;
                }
                params.print(printer)?;
                printer.write_str(" : ")?;
                typ.print(printer)?;
                printer.write_str(" =")?;
                for (index, clause) in clauses.iter().enumerate() {
                    write!(printer, "\n\n  clause {index} : ")?;
                    clause.print(printer)?;
                }
                if let Some(else_clause) = else_clause {
                    printer.write_str("\n\n  clause -1 : ")?;
                    else_clause.print(printer)?;
                }
                Ok(())
            }
        }
    }
}

impl Print for [Def] {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        for (index, definition) in self.iter().enumerate() {
            if index != 0 {
                printer.write_str("\n\n")?;
            }
            definition.print(printer)?;
        }
        Ok(())
    }
}

// - Specifications

impl Print for Spec {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        self.as_slice().print(printer)
    }
}
