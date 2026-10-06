//! Explicit rendering of prepared expressions and arguments
//!
//! The caller supplies the arena for every interned notation.

use super::prepare::ast::*;
use crate::{
    lang::{
        data::notation::MixopArena,
        traits::print::{Print, Printer},
    },
    util::text::escape_text,
};
use std::fmt::{self, Write};

/// Prints a prepared exp with its notation arena.
pub fn print_exp(arena_mixop: &MixopArena, exp: &Exp, printer: &mut Printer<'_>) -> fmt::Result {
    match &exp.node {
        ExpKind::Bool(value) => write!(printer, "{value}"),
        ExpKind::Num(value) => value.print(printer),
        ExpKind::Text(text) => write!(printer, "\"{}\"", escape_text(text)),
        ExpKind::Id(id) => id.print(printer),
        ExpKind::Un(op, _, exp) => {
            op.print(printer)?;
            print_exp(arena_mixop, exp, printer)
        }
        ExpKind::Bin(op, _, exp_l, exp_r) => {
            printer.write_char('(')?;
            print_exp(arena_mixop, exp_l, printer)?;
            printer.write_char(' ')?;
            op.print(printer)?;
            printer.write_char(' ')?;
            print_exp(arena_mixop, exp_r, printer)?;
            printer.write_char(')')
        }
        ExpKind::Cmp(op, _, exp_l, exp_r) => {
            printer.write_char('(')?;
            print_exp(arena_mixop, exp_l, printer)?;
            printer.write_char(' ')?;
            op.print(printer)?;
            printer.write_char(' ')?;
            print_exp(arena_mixop, exp_r, printer)?;
            printer.write_char(')')
        }
        ExpKind::UpCast(typ, exp) | ExpKind::DownCast(typ, exp) => {
            print_exp(arena_mixop, exp, printer)?;
            printer.write_str(" as ")?;
            typ.print(printer)
        }
        ExpKind::Sub(exp, typ, _) => {
            print_exp(arena_mixop, exp, printer)?;
            printer.write_str(" <: ")?;
            typ.print(printer)
        }
        ExpKind::Match(exp, pattern) => {
            print_exp(arena_mixop, exp, printer)?;
            printer.write_str(" matches ")?;
            print_pattern(arena_mixop, pattern, printer)
        }
        ExpKind::Tuple(exps) => {
            printer.write_char('(')?;
            print_exps(arena_mixop, exps, printer)?;
            printer.write_char(')')
        }
        ExpKind::Case(not_exp) => not_exp.print_in_with(arena_mixop, printer, |exp, printer| {
            print_exp(arena_mixop, exp, printer)
        }),
        ExpKind::Str(fields) => {
            printer.write_char('{')?;
            for (index, ExpField { atom, exp }) in fields.iter().enumerate() {
                if index != 0 {
                    printer.write_str(", ")?;
                }
                atom.print(printer)?;
                printer.write_char(' ')?;
                print_exp(arena_mixop, exp, printer)?;
            }
            printer.write_char('}')
        }
        ExpKind::Opt(exp) => {
            printer.write_str("?(")?;
            if let Some(exp) = exp {
                print_exp(arena_mixop, exp, printer)?;
            }
            printer.write_char(')')
        }
        ExpKind::List(exps) => {
            printer.write_char('[')?;
            print_exps(arena_mixop, exps, printer)?;
            printer.write_char(']')
        }
        ExpKind::Cons(exp_head, exp_tail) => {
            print_exp(arena_mixop, exp_head, printer)?;
            printer.write_str(" :: ")?;
            print_exp(arena_mixop, exp_tail, printer)
        }
        ExpKind::Cat(exp_l, exp_r) => {
            print_exp(arena_mixop, exp_l, printer)?;
            printer.write_str(" ++ ")?;
            print_exp(arena_mixop, exp_r, printer)
        }
        ExpKind::Mem(exp_elem, exp_set) => {
            print_exp(arena_mixop, exp_elem, printer)?;
            printer.write_str(" <- ")?;
            print_exp(arena_mixop, exp_set, printer)
        }
        ExpKind::Len(exp) => {
            printer.write_char('|')?;
            print_exp(arena_mixop, exp, printer)?;
            printer.write_char('|')
        }
        ExpKind::Dot(exp, atom) => {
            print_exp(arena_mixop, exp, printer)?;
            printer.write_char('.')?;
            atom.print(printer)
        }
        ExpKind::Idx(exp_base, exp_idx) => {
            print_exp(arena_mixop, exp_base, printer)?;
            printer.write_char('[')?;
            print_exp(arena_mixop, exp_idx, printer)?;
            printer.write_char(']')
        }
        ExpKind::Slice(exp_base, exp_idx, exp_len) => {
            print_exp(arena_mixop, exp_base, printer)?;
            printer.write_char('[')?;
            print_exp(arena_mixop, exp_idx, printer)?;
            printer.write_str(" : ")?;
            print_exp(arena_mixop, exp_len, printer)?;
            printer.write_char(']')
        }
        ExpKind::Upd(exp_base, path, exp_field) => {
            print_exp(arena_mixop, exp_base, printer)?;
            printer.write_char('[')?;
            print_path(arena_mixop, path, printer)?;
            printer.write_str(" = ")?;
            print_exp(arena_mixop, exp_field, printer)?;
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
            print_args(arena_mixop, args, printer)
        }
        ExpKind::Iter(exp, exp_iter) => {
            print_exp(arena_mixop, exp, printer)?;
            exp_iter.print(printer)
        }
    }
}

/// Prints a prepared path with its notation arena.
fn print_path(arena_mixop: &MixopArena, path: &Path, printer: &mut Printer<'_>) -> fmt::Result {
    match &path.node {
        PathKind::Root => Ok(()),
        PathKind::Idx(path, exp_idx) => {
            print_path(arena_mixop, path, printer)?;
            printer.write_char('[')?;
            print_exp(arena_mixop, exp_idx, printer)?;
            printer.write_char(']')
        }
        PathKind::Slice(path, exp_idx, exp_len) => {
            print_path(arena_mixop, path, printer)?;
            printer.write_char('[')?;
            print_exp(arena_mixop, exp_idx, printer)?;
            printer.write_str(" : ")?;
            print_exp(arena_mixop, exp_len, printer)?;
            printer.write_char(']')
        }
        PathKind::Dot(path, atom) if matches!(path.node, PathKind::Root) => atom.print(printer),
        PathKind::Dot(path, atom) => {
            print_path(arena_mixop, path, printer)?;
            printer.write_char('.')?;
            atom.print(printer)
        }
    }
}

/// Prints a prepared arg with its notation arena.
pub fn print_arg(arena_mixop: &MixopArena, arg: &Arg, printer: &mut Printer<'_>) -> fmt::Result {
    match &arg.node {
        ArgKind::Exp(exp) => print_exp(arena_mixop, exp, printer),
        ArgKind::Def(id) => {
            printer.write_char('$')?;
            id.print(printer)
        }
    }
}

/// Renders a prepared expression with its notation arena.
///
/// ```
/// use p4spectec::{
///     interp::shared::{prepare::ast::Exp, print::exp_to_string},
///     lang::data::notation::MixopArena,
/// };
/// fn with_arena(arena: &MixopArena, exp: &Exp) -> String {
///     exp_to_string(arena, exp)
/// }
/// ```
pub fn exp_to_string(arena_mixop: &MixopArena, exp: &Exp) -> String {
    let mut output = String::new();
    print_exp(arena_mixop, exp, &mut Printer::new(&mut output))
        .expect("writing to a String cannot fail");
    output
}

/// Renders a prepared argument with its notation arena.
pub fn arg_to_string(arena_mixop: &MixopArena, arg: &Arg) -> String {
    let mut output = String::new();
    print_arg(arena_mixop, arg, &mut Printer::new(&mut output))
        .expect("writing to a String cannot fail");
    output
}

fn print_exps(arena_mixop: &MixopArena, exps: &[Exp], printer: &mut Printer<'_>) -> fmt::Result {
    for (index, exp) in exps.iter().enumerate() {
        if index != 0 {
            printer.write(", ")?;
        }
        print_exp(arena_mixop, exp, printer)?;
    }
    Ok(())
}

fn print_args(arena_mixop: &MixopArena, args: &[Arg], printer: &mut Printer<'_>) -> fmt::Result {
    if args.is_empty() {
        return Ok(());
    }
    printer.write("(")?;
    for (index, arg) in args.iter().enumerate() {
        if index != 0 {
            printer.write(", ")?;
        }
        print_arg(arena_mixop, arg, printer)?;
    }
    printer.write(")")
}
/// Prints a prepared pattern using the supplied arena.
pub fn print_pattern(
    arena_mixop: &MixopArena,
    pattern: &Pattern,
    printer: &mut Printer<'_>,
) -> fmt::Result {
    match pattern {
        Pattern::Case(mixop) => crate::lang::data::notation::walk::print_flat_with(
            arena_mixop,
            arena_mixop.kind(*mixop),
            printer,
            |_, printer| printer.write("%"),
        ),
        Pattern::List(ListPattern::Cons) => printer.write_str("_ :: _"),
        Pattern::List(ListPattern::Fixed(length)) => write!(printer, "[ _/{length} ]"),
        Pattern::List(ListPattern::Nil) => printer.write_str("[]"),
        Pattern::Opt(OptPattern::Some) => printer.write_str("(_)"),
        Pattern::Opt(OptPattern::None) => printer.write_str("()"),
    }
}
