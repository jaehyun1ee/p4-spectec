//! Rendering shared values through their arena
//!
//! `print_value` resolves bodies and case mixops through the arena.
//! Nested aggregates preserve field order and indentation.

use std::fmt::{self, Write};

use crate::util::text::escape_text;

use crate::lang::{
    data::arena::Arena,
    traits::print::{Print, Printer},
};

use super::flat::{Value, ValueKind};

// = Flat values

/// Prints a value in full, resolving handles through the arena.
pub fn print_value(arena: &Arena, value: &Value, printer: &mut Printer<'_>) -> fmt::Result {
    print_value_inner(arena, value, printer, 0)
}

/// Prints a value with nested aggregates indented by `level`.
fn print_value_inner(
    arena: &Arena,
    value: &Value,
    printer: &mut Printer<'_>,
    level: usize,
) -> fmt::Result {
    match arena.kind(value) {
        ValueKind::Bool(value) => write!(printer, "{value}"),
        ValueKind::Num(value) => value.print(printer),
        ValueKind::Text(text) => printer.write_str(&escape_text(text)),
        // Empty structs stay on one line
        ValueKind::Struct(value_fields) if value_fields.is_empty() => printer.write_str("{}"),
        // One field per line, indented one level deeper
        ValueKind::Struct(value_fields) => {
            printer.write_str("{\n")?;
            for (idx, (atom, value)) in value_fields.iter().enumerate() {
                if idx != 0 {
                    printer.write_str(";\n")?;
                }
                printer.write_str(&indent(level + 1))?;
                atom.print(printer)?;
                printer.write_char(' ')?;
                print_value_inner(arena, value, printer, level + 1)?;
            }
            printer.write_char('\n')?;
            printer.write_str(&indent(level))?;
            printer.write_char('}')
        }
        ValueKind::Case(value_case) => {
            value_case.print_in_with(arena.arena_mixop(), printer, |value, printer| {
                print_value_inner(arena, value, printer, level + 1)
            })
        }
        ValueKind::Tuple(values) => {
            printer.write_char('(')?;
            for (idx, value) in values.iter().enumerate() {
                if idx != 0 {
                    printer.write_str(", ")?;
                }
                print_value_inner(arena, value, printer, level + 1)?;
            }
            printer.write_char(')')
        }
        ValueKind::Opt(Some(value)) => {
            printer.write_str("Some(")?;
            print_value_inner(arena, value, printer, level + 1)?;
            printer.write_char(')')
        }
        ValueKind::Opt(None) => printer.write_str("None"),
        // Empty lists stay on one line
        ValueKind::List(values) if values.is_empty() => printer.write_str("[]"),
        // One element per line, indented one level deeper
        ValueKind::List(values) => {
            printer.write_str("[\n")?;
            for (idx, value) in values.iter().enumerate() {
                if idx != 0 {
                    printer.write_str(",\n")?;
                }
                printer.write_str(&indent(level + 1))?;
                print_value_inner(arena, value, printer, level + 1)?;
            }
            printer.write_char('\n')?;
            printer.write_str(&indent(level))?;
            printer.write_char(']')
        }
        ValueKind::Func(id) => {
            printer.write_char('$')?;
            printer.write_str(&id.node)
        }
        ValueKind::Extern(_) => printer.write_str("extern"),
    }
}

// = Indentation

fn indent(level: usize) -> String {
    "  ".repeat(level)
}
