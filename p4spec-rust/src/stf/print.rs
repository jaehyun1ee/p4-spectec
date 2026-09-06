//! Stable STF diagnostic text output
//!
//! Each syntax type writes directly to the shared language printer. Compound
//! values delegate to their components, and programs separate statements with
//! newlines. For example, an action named `drop` renders as `"drop"()`.

use std::fmt;

use crate::lang::traits::print::{Print, Printer};

use super::ast::{Action, Condition, CounterKind, IdOrIndex, Match, MatchKind, Program, Statement};

// == Lexical helpers

pub fn convert_dollar_to_brackets(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut characters = value.char_indices().peekable();
    while let Some((_, character)) = characters.next() {
        if character != '$'
            || !characters
                .peek()
                .is_some_and(|(_, next)| next.is_ascii_digit())
        {
            result.push(character);
            continue;
        }
        result.push('[');
        while let Some((_, digit)) = characters.next_if(|(_, next)| next.is_ascii_digit()) {
            result.push(digit);
        }
        result.push(']');
    }
    result
}

fn write_quoted(printer: &mut Printer<'_>, value: &str) -> fmt::Result {
    printer.write_fmt(format_args!("\"{value}\""))
}

fn write_argument(printer: &mut Printer<'_>, id: &str, number: &str) -> fmt::Result {
    write_quoted(printer, id)?;
    printer.write_fmt(format_args!(":{number}"))
}

fn write_match(printer: &mut Printer<'_>, (name, kind): &Match) -> fmt::Result {
    write_quoted(printer, name)?;
    printer.write(":")?;
    kind.print(printer)
}

// == Compound syntax

impl Print for Action {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        write_quoted(printer, &self.name)?;
        printer.write("(")?;
        for (index, (id, number)) in self.args.iter().enumerate() {
            if index != 0 {
                printer.write(",")?;
            }
            write_argument(printer, id, number)?;
        }
        printer.write(")")
    }
}

impl Print for MatchKind {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match self {
            Self::Number(number) => printer.write(number),
            Self::Slash(number_l, number_r) => {
                printer.write_fmt(format_args!("{number_l}/{number_r}"))
            }
        }
    }
}

impl Print for IdOrIndex {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match self {
            Self::Id(id) => printer.write(id),
            Self::Index(number) => printer.write(number),
        }
    }
}

impl Print for Condition {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        let text = match self {
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::Le => "<=",
            Self::Lt => "<",
            Self::Ge => ">=",
            Self::Gt => ">",
        };
        printer.write(text)
    }
}

impl Print for CounterKind {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        let text = match self {
            Self::Bytes => "bytes",
            Self::Packets => "packets",
        };
        printer.write(text)
    }
}

// == Statements and programs

impl Print for Statement {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match self {
            Self::Wait => printer.write("wait"),
            Self::RemoveAll => printer.write("remove_all"),
            Self::Expect(port, expected, exact) => {
                printer.write_fmt(format_args!("expect {port}"))?;
                if expected.is_some() || *exact {
                    printer.write(" ")?;
                }
                if let Some(expected) = expected {
                    printer.write(expected)?;
                }
                if *exact {
                    printer.write("$")?;
                }
                Ok(())
            }
            Self::Packet(port, packet) => printer.write_fmt(format_args!("packet {port} {packet}")),
            Self::NoPacket => printer.write("no_packet"),
            Self::Add {
                table,
                priority,
                matches,
                action,
                id,
            } => {
                printer.write("add ")?;
                write_quoted(printer, table)?;
                if let Some(priority) = priority {
                    printer.write_fmt(format_args!(" {priority}"))?;
                }
                for entry in matches {
                    printer.write(" ")?;
                    write_match(printer, entry)?;
                }
                printer.write(" ")?;
                action.print(printer)?;
                if let Some(id) = id {
                    printer.write(" ")?;
                    write_quoted(printer, id)?;
                }
                Ok(())
            }
            Self::SetDefault { table, action } => {
                printer.write("setdefault ")?;
                write_quoted(printer, table)?;
                printer.write(" ")?;
                action.print(printer)
            }
            Self::CheckCounter { id, target, check } => {
                printer.write("check_counter ")?;
                write_quoted(printer, id)?;
                printer.write("(")?;
                target.print(printer)?;
                printer.write(")")?;
                if let Some(counter) = check.0 {
                    printer.write(" ")?;
                    counter.print(printer)?;
                }
                printer.write(" ")?;
                check.1.print(printer)?;
                printer.write_fmt(format_args!(" {}", check.2))
            }
            Self::MirroringAdd(session, port) => {
                printer.write_fmt(format_args!("mirroring_add {session} {port}"))
            }
            Self::MirroringAddMc(session, id) => {
                printer.write_fmt(format_args!("mirroring_add_mc {session} {id}"))
            }
            Self::MirroringGet(session) => {
                printer.write_fmt(format_args!("mirroring_get {session}"))
            }
            Self::McGroupCreate(id) => printer.write_fmt(format_args!("mc_mgrp_create {id}")),
            Self::McNodeCreate(id, ports) => {
                printer.write_fmt(format_args!("mc_node_create {id} "))?;
                for (index, port) in ports.iter().enumerate() {
                    if index != 0 {
                        printer.write(" ")?;
                    }
                    printer.write(port)?;
                }
                Ok(())
            }
            Self::McNodeAssociate(id, handle) => {
                printer.write_fmt(format_args!("mc_mgrp_associate {id} {handle}"))
            }
            Self::RegisterRead(name, index) => {
                printer.write("register_read ")?;
                write_quoted(printer, name)?;
                printer.write_fmt(format_args!(" {index}"))
            }
            Self::RegisterWrite(name, index, number) => {
                printer.write("register_write ")?;
                write_quoted(printer, name)?;
                printer.write_fmt(format_args!(" {index} {number}"))
            }
            Self::RegisterReset(name) => {
                printer.write("register_reset ")?;
                write_quoted(printer, name)
            }
        }
    }
}

impl Print for Program {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        for (index, statement) in self.iter().enumerate() {
            if index != 0 {
                printer.newline()?;
            }
            statement.node.print(printer)?;
        }
        Ok(())
    }
}
