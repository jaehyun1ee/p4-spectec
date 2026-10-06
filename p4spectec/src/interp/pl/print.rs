//! Explicit rendering of prepared expressions and arguments
//!
//! The caller supplies the arena for every interned notation.

use std::fmt::{self, Write};

use crate::util::text::escape_text;

use crate::lang::{
    data::notation::MixopArena,
    traits::print::{Print, Printer},
};

use crate::lang::pl::prepared::*;

use crate::interp::shared::print::print_pattern;

/// Prints a prepared exp with its notation arena.
pub fn print_exp(arena_mixop: &MixopArena, exp: &Exp, printer: &mut Printer<'_>) -> fmt::Result {
    match &exp.node.node {
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
            printer.write_char('(')?;
            print_exp(arena_mixop, exp, printer)?;
            printer.write_str(" as ")?;
            typ.print(printer)?;
            printer.write_char(')')
        }
        ExpKind::Sub(exp, typ, _) => {
            printer.write_char('(')?;
            print_exp(arena_mixop, exp, printer)?;
            printer.write_str(" has type ")?;
            typ.print(printer)?;
            printer.write_char(')')
        }
        ExpKind::Match(exp, pattern) => {
            printer.write_char('(')?;
            print_exp(arena_mixop, exp, printer)?;
            printer.write_str(" matches pattern ")?;
            print_pattern(arena_mixop, pattern, printer)?;
            printer.write_char(')')
        }
        ExpKind::Tuple(exps) => {
            printer.write_char('(')?;
            print_exps(arena_mixop, exps, printer)?;
            printer.write_char(')')
        }
        ExpKind::Case(not_exp) => {
            printer.write_char('(')?;
            not_exp.print_in_with(arena_mixop, printer, |exp, printer| {
                print_exp(arena_mixop, exp, printer)
            })?;
            printer.write_char(')')
        }
        ExpKind::Str(fields) => {
            printer.write_char('{')?;
            for (index, (atom, exp)) in fields.iter().enumerate() {
                if index > 0 {
                    printer.write_str(", ")?;
                }
                atom.print(printer)?;
                printer.write_char(' ')?;
                print_exp(arena_mixop, exp, printer)?;
            }
            printer.write_char('}')
        }
        ExpKind::Opt(Some(exp)) => {
            printer.write_str("?(")?;
            print_exp(arena_mixop, exp, printer)?;
            printer.write_char(')')
        }
        ExpKind::Opt(None) => printer.write_str("?()"),
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
            printer.write_str(" is in ")?;
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
        ExpKind::Iter(exp, iter_exp) => {
            printer.write_char('(')?;
            print_exp(arena_mixop, exp, printer)?;
            printer.write_char(')')?;
            std::slice::from_ref(iter_exp).print(printer)
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
        // A field of the root prints bare
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
