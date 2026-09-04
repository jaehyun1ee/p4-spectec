//! Stable STF diagnostic text output.
//!
//! Leaf values are normalized first, then statements are rendered in OCaml
//! variant order and programs join them with newlines. For example, an action
//! named `drop` with no arguments renders as `"drop"()`.

use std::{fmt, fmt::Write};

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

fn action(action: &Action) -> String {
    let args = action
        .args
        .iter()
        .map(|(id, number)| format!("\"{id}\":{number}"))
        .collect::<Vec<_>>()
        .join(",");
    format!("\"{}\"({args})", action.name)
}

fn match_kind(kind: &MatchKind) -> String {
    match kind {
        MatchKind::Number(number) => number.clone(),
        MatchKind::Slash(left, right) => format!("{left}/{right}"),
    }
}

fn match_value((name, kind): &Match) -> String {
    format!("\"{name}\":{}", match_kind(kind))
}

fn condition(condition: Condition) -> &'static str {
    match condition {
        Condition::Eq => "==",
        Condition::Ne => "!=",
        Condition::Le => "<=",
        Condition::Lt => "<",
        Condition::Ge => ">=",
        Condition::Gt => ">",
    }
}

fn counter(counter: CounterKind) -> &'static str {
    match counter {
        CounterKind::Bytes => "bytes",
        CounterKind::Packets => "packets",
    }
}

// == Statements and programs

pub fn statement(statement: &Statement) -> String {
    match statement {
        Statement::Wait => "wait".to_owned(),
        Statement::RemoveAll => "remove_all".to_owned(),
        Statement::Expect(port, expected, exact) => {
            let expected = expected.as_deref().unwrap_or("");
            let exact = if *exact { "$" } else { "" };
            format!("expect {port} {expected}{exact}")
                .trim_end()
                .to_owned()
        }
        Statement::Packet(port, packet) => format!("packet {port} {packet}"),
        Statement::NoPacket => "no_packet".to_owned(),
        Statement::Add {
            table,
            priority,
            matches,
            action: invocation,
            id,
        } => {
            let mut result = format!("add \"{table}\"");
            if let Some(priority) = priority {
                write!(result, " {priority}").expect("writing to String cannot fail");
            }
            for entry in matches {
                write!(result, " {}", match_value(entry)).expect("writing to String cannot fail");
            }
            write!(result, " {}", action(invocation)).expect("writing to String cannot fail");
            if let Some(id) = id {
                write!(result, " \"{id}\"").expect("writing to String cannot fail");
            }
            result
        }
        Statement::SetDefault {
            table,
            action: invocation,
        } => format!("setdefault \"{table}\" {}", action(invocation)),
        Statement::CheckCounter { id, target, check } => {
            let target = match target {
                IdOrIndex::Id(id) => id,
                IdOrIndex::Index(number) => number,
            };
            let counter = check
                .0
                .map(|value| format!(" {}", counter(value)))
                .unwrap_or_default();
            format!(
                "check_counter \"{id}\"({target}){counter} {} {}",
                condition(check.1),
                check.2
            )
        }
        Statement::MirroringAdd(session, port) => format!("mirroring_add {session} {port}"),
        Statement::MirroringAddMc(session, id) => format!("mirroring_add_mc {session} {id}"),
        Statement::MirroringGet(session) => format!("mirroring_get {session}"),
        Statement::McGroupCreate(id) => format!("mc_mgrp_create {id}"),
        Statement::McNodeCreate(id, ports) => {
            format!("mc_node_create {id} {}", ports.join(" "))
        }
        Statement::McNodeAssociate(id, handle) => {
            format!("mc_mgrp_associate {id} {handle}")
        }
        Statement::RegisterRead(name, index) => format!("register_read \"{name}\" {index}"),
        Statement::RegisterWrite(name, index, number) => {
            format!("register_write \"{name}\" {index} {number}")
        }
        Statement::RegisterReset(name) => format!("register_reset \"{name}\""),
    }
}

pub fn program(program: &Program) -> String {
    program
        .iter()
        .map(|statement| self::statement(&statement.node))
        .collect::<Vec<_>>()
        .join("\n")
}

// == Language printing

impl Print for Statement {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.write(&statement(self))
    }
}

impl Print for Program {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.write(&program(self))
    }
}
