//! Rendering shared values through their arena

use std::fmt::{self, Write};

use crate::util::text::escape_text;

use crate::lang::{
    data::arena::Arena,
    traits::print::{Print, Printer},
};

use super::flat::{Value, ValueCase, ValueKind};

// - Values

/// Prints a value in full, resolving handles through the arena.
pub fn print_value(arena: &Arena, value: &Value, printer: &mut Printer<'_>) -> fmt::Result {
    write_value_with(arena, printer, value, 0)
}

/// Prints a value with nested aggregates indented by `level`.
fn write_value_with(
    arena: &Arena,
    output: &mut Printer<'_>,
    value: &Value,
    level: usize,
) -> fmt::Result {
    match arena.kind(value) {
        ValueKind::Bool(value) => write!(output, "{value}"),
        ValueKind::Num(value) => value.print(output),
        ValueKind::Text(text) => output.write_str(&escape_text(text)),
        // Empty structs stay on one line
        ValueKind::Struct(fields) if fields.is_empty() => output.write_str("{}"),
        // One field per line, indented one level deeper
        ValueKind::Struct(fields) => {
            output.write_str("{\n")?;
            for (index, (atom, value)) in fields.iter().enumerate() {
                if index != 0 {
                    output.write_str(";\n")?;
                }
                output.write_str(&indent(level + 1))?;
                atom.print(output)?;
                output.write_char(' ')?;
                write_value_with(arena, output, value, level + 1)?;
            }
            output.write_char('\n')?;
            output.write_str(&indent(level))?;
            output.write_char('}')
        }
        ValueKind::Case(case) => write_case_with(arena, output, case, level),
        ValueKind::Tuple(values) => {
            output.write_char('(')?;
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.write_str(", ")?;
                }
                write_value_with(arena, output, value, level + 1)?;
            }
            output.write_char(')')
        }
        ValueKind::Opt(Some(value)) => {
            output.write_str("Some(")?;
            write_value_with(arena, output, value, level + 1)?;
            output.write_char(')')
        }
        ValueKind::Opt(None) => output.write_str("None"),
        // Empty lists stay on one line
        ValueKind::List(values) if values.is_empty() => output.write_str("[]"),
        // One element per line, indented one level deeper
        ValueKind::List(values) => {
            output.write_str("[\n")?;
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.write_str(",\n")?;
                }
                output.write_str(&indent(level + 1))?;
                write_value_with(arena, output, value, level + 1)?;
            }
            output.write_char('\n')?;
            output.write_str(&indent(level))?;
            output.write_char(']')
        }
        ValueKind::Func(id) => {
            output.write_char('$')?;
            output.write_str(&id.node)
        }
        ValueKind::Extern(_) => output.write_str("extern"),
    }
}

/// Prints a variant value with its arguments filled into the skeleton.
fn write_case_with(
    arena: &Arena,
    output: &mut Printer<'_>,
    value_case: &ValueCase,
    level: usize,
) -> fmt::Result {
    value_case.print_in_with(arena.arena_mixop(), output, |value, output| {
        write_value_with(arena, output, value, level + 1)
    })
}

fn indent(level: usize) -> String {
    "  ".repeat(level)
}
