//! Borrowed prepared syntax rendered through the Print trait
//!
//! Each view pairs prepared syntax with its notation arena.
//! `Print::print` resolves mixops while traversing the syntax;
//! `Print::to_string` supplies the common string-rendering entry point.

use std::fmt::{self, Write};

use crate::util::text::escape_text;

use crate::lang::{
    data::notation::MixopArena,
    traits::print::{Print, Printer},
};

use crate::lang::il::prepared::*;

// == Borrowed views

/// Borrows a prepared expression and its notation arena for printing.
#[derive(Clone, Copy, Debug)]
pub struct ExpRef<'a> {
    /// The arena that owns the notation handles.
    pub arena_mixop: &'a MixopArena,
    /// The prepared expression to render.
    pub exp: &'a Exp,
}

/// Borrows a prepared argument and its notation arena for printing.
#[derive(Clone, Copy, Debug)]
pub struct ArgRef<'a> {
    /// The arena that owns the notation handles.
    pub arena_mixop: &'a MixopArena,
    /// The prepared argument to render.
    pub arg: &'a Arg,
}

/// Borrows a prepared pattern and its notation arena for printing.
#[derive(Clone, Copy, Debug)]
pub struct PatternRef<'a> {
    /// The arena that owns the notation handles.
    pub arena_mixop: &'a MixopArena,
    /// The prepared pattern to render.
    pub pattern: &'a Pattern,
}

// == Printing

// - Expressions

impl Print for ExpRef<'_> {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        let arena_mixop = self.arena_mixop;
        let exp = self.exp;
        match &exp.node {
            ExpKind::Bool(value) => write!(printer, "{value}"),
            ExpKind::Num(value) => value.print(printer),
            ExpKind::Text(text) => write!(printer, "\"{}\"", escape_text(text)),
            ExpKind::Id(id) => id.print(printer),
            ExpKind::Un(op, _, exp) => {
                op.print(printer)?;
                ExpRef { arena_mixop, exp }.print(printer)
            }
            ExpKind::Bin(op, _, exp_l, exp_r) => {
                printer.write_char('(')?;
                ExpRef { arena_mixop, exp: exp_l }.print(printer)?;
                printer.write_char(' ')?;
                op.print(printer)?;
                printer.write_char(' ')?;
                ExpRef { arena_mixop, exp: exp_r }.print(printer)?;
                printer.write_char(')')
            }
            ExpKind::Cmp(op, _, exp_l, exp_r) => {
                printer.write_char('(')?;
                ExpRef { arena_mixop, exp: exp_l }.print(printer)?;
                printer.write_char(' ')?;
                op.print(printer)?;
                printer.write_char(' ')?;
                ExpRef { arena_mixop, exp: exp_r }.print(printer)?;
                printer.write_char(')')
            }
            ExpKind::UpCast(typ, exp) | ExpKind::DownCast(typ, exp) => {
                ExpRef { arena_mixop, exp }.print(printer)?;
                printer.write_str(" as ")?;
                typ.print(printer)
            }
            ExpKind::Sub(exp, typ, _) => {
                ExpRef { arena_mixop, exp }.print(printer)?;
                printer.write_str(" <: ")?;
                typ.print(printer)
            }
            ExpKind::Match(exp, pattern) => {
                ExpRef { arena_mixop, exp }.print(printer)?;
                printer.write_str(" matches ")?;
                PatternRef { arena_mixop, pattern }.print(printer)
            }
            ExpKind::Tuple(exps) => {
                printer.write_char('(')?;
                ExpRef::print_exps(arena_mixop, exps, printer)?;
                printer.write_char(')')
            }
            ExpKind::Case(not_exp) => not_exp.print_with(arena_mixop, printer, |exp, printer| {
                ExpRef { arena_mixop, exp }.print(printer)
            }),
            ExpKind::Str(exp_fields) => {
                printer.write_char('{')?;
                for (idx, ExpField { atom, exp }) in exp_fields.iter().enumerate() {
                    if idx != 0 {
                        printer.write_str(", ")?;
                    }
                    atom.print(printer)?;
                    printer.write_char(' ')?;
                    ExpRef { arena_mixop, exp }.print(printer)?;
                }
                printer.write_char('}')
            }
            ExpKind::Opt(exp) => {
                printer.write_str("?(")?;
                if let Some(exp) = exp {
                    ExpRef { arena_mixop, exp }.print(printer)?;
                }
                printer.write_char(')')
            }
            ExpKind::List(exps) => {
                printer.write_char('[')?;
                ExpRef::print_exps(arena_mixop, exps, printer)?;
                printer.write_char(']')
            }
            ExpKind::Cons(exp_head, exp_tail) => {
                ExpRef { arena_mixop, exp: exp_head }.print(printer)?;
                printer.write_str(" :: ")?;
                ExpRef { arena_mixop, exp: exp_tail }.print(printer)
            }
            ExpKind::Cat(exp_l, exp_r) => {
                ExpRef { arena_mixop, exp: exp_l }.print(printer)?;
                printer.write_str(" ++ ")?;
                ExpRef { arena_mixop, exp: exp_r }.print(printer)
            }
            ExpKind::Mem(exp_elem, exp_set) => {
                ExpRef { arena_mixop, exp: exp_elem }.print(printer)?;
                printer.write_str(" <- ")?;
                ExpRef { arena_mixop, exp: exp_set }.print(printer)
            }
            ExpKind::Len(exp) => {
                printer.write_char('|')?;
                ExpRef { arena_mixop, exp }.print(printer)?;
                printer.write_char('|')
            }
            ExpKind::Dot(exp, atom) => {
                ExpRef { arena_mixop, exp }.print(printer)?;
                printer.write_char('.')?;
                atom.print(printer)
            }
            ExpKind::Idx(exp_base, exp_idx) => {
                ExpRef { arena_mixop, exp: exp_base }.print(printer)?;
                printer.write_char('[')?;
                ExpRef { arena_mixop, exp: exp_idx }.print(printer)?;
                printer.write_char(']')
            }
            ExpKind::Slice(exp_base, exp_idx, exp_len) => {
                ExpRef { arena_mixop, exp: exp_base }.print(printer)?;
                printer.write_char('[')?;
                ExpRef { arena_mixop, exp: exp_idx }.print(printer)?;
                printer.write_str(" : ")?;
                ExpRef { arena_mixop, exp: exp_len }.print(printer)?;
                printer.write_char(']')
            }
            ExpKind::Upd(exp_base, path, exp_field) => {
                ExpRef { arena_mixop, exp: exp_base }.print(printer)?;
                printer.write_char('[')?;
                ExpRef::print_path(arena_mixop, path, printer)?;
                printer.write_str(" = ")?;
                ExpRef { arena_mixop, exp: exp_field }.print(printer)?;
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
                ArgRef::print_args(arena_mixop, args, printer)
            }
            ExpKind::Iter(exp, exp_iter) => {
                ExpRef { arena_mixop, exp }.print(printer)?;
                exp_iter.print(printer)
            }
        }
    }
}

impl ExpRef<'_> {
    /// Prints expressions in order, separated by commas.
    fn print_exps(
        arena_mixop: &MixopArena,
        exps: &[Exp],
        printer: &mut Printer<'_>,
    ) -> fmt::Result {
        for (idx, exp) in exps.iter().enumerate() {
            if idx != 0 {
                printer.write(", ")?;
            }
            ExpRef { arena_mixop, exp }.print(printer)?;
        }
        Ok(())
    }

    /// Prints a path, resolving its index and slice expressions.
    fn print_path(arena_mixop: &MixopArena, path: &Path, printer: &mut Printer<'_>) -> fmt::Result {
        match &path.node {
            PathKind::Root => Ok(()),
            PathKind::Idx(path, exp_idx) => {
                ExpRef::print_path(arena_mixop, path, printer)?;
                printer.write_char('[')?;
                ExpRef { arena_mixop, exp: exp_idx }.print(printer)?;
                printer.write_char(']')
            }
            PathKind::Slice(path, exp_idx, exp_len) => {
                ExpRef::print_path(arena_mixop, path, printer)?;
                printer.write_char('[')?;
                ExpRef { arena_mixop, exp: exp_idx }.print(printer)?;
                printer.write_str(" : ")?;
                ExpRef { arena_mixop, exp: exp_len }.print(printer)?;
                printer.write_char(']')
            }
            PathKind::Dot(path, atom) if matches!(path.node, PathKind::Root) => atom.print(printer),
            PathKind::Dot(path, atom) => {
                ExpRef::print_path(arena_mixop, path, printer)?;
                printer.write_char('.')?;
                atom.print(printer)
            }
        }
    }
}

// - Arguments

impl Print for ArgRef<'_> {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        let arena_mixop = self.arena_mixop;
        let arg = self.arg;
        match &arg.node {
            ArgKind::Exp(exp) => ExpRef { arena_mixop, exp }.print(printer),
            ArgKind::Def(id) => {
                printer.write_char('$')?;
                id.print(printer)
            }
        }
    }
}

impl ArgRef<'_> {
    /// Prints nonempty argument lists in parentheses.
    fn print_args(
        arena_mixop: &MixopArena,
        args: &[Arg],
        printer: &mut Printer<'_>,
    ) -> fmt::Result {
        if args.is_empty() {
            return Ok(());
        }
        printer.write("(")?;
        for (idx, arg) in args.iter().enumerate() {
            if idx != 0 {
                printer.write(", ")?;
            }
            ArgRef { arena_mixop, arg }.print(printer)?;
        }
        printer.write(")")
    }
}

// - Patterns

impl Print for PatternRef<'_> {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        let arena_mixop = self.arena_mixop;
        let pattern = self.pattern;
        match pattern {
            Pattern::Case(mixop) => {
                mixop.print_with(arena_mixop, printer, |_, printer| printer.write("%"))
            }
            Pattern::List(ListPattern::Cons) => printer.write_str("_ :: _"),
            Pattern::List(ListPattern::Fixed(length)) => write!(printer, "[ _/{length} ]"),
            Pattern::List(ListPattern::Nil) => printer.write_str("[]"),
            Pattern::Opt(OptPattern::Some) => printer.write_str("(_)"),
            Pattern::Opt(OptPattern::None) => printer.write_str("()"),
        }
    }
}
