//! Executes transformed STF statements against one runner and per-run state

use super::{
    architecture::Architecture,
    io::{self, Expectation, Transmission},
    spec_impl::unpack,
    state::SimState,
    table,
};
use crate::{
    interface::p4::{error::P4Error, parse},
    interp::al::error::Error as InterpError,
    lang::{
        common::source::{Phrase, Span},
        data::{
            typ,
            value::{Value, ValueArena, make},
        },
        traits::print::Print,
    },
    runner::{Interface, Interpreter, Runner},
    stf::{
        self,
        ast::{Action, MatchKind, Statement, TableMatch},
    },
};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("syntax error: {0}")]
    P4Syntax(#[from] P4Error),
    #[error("runtime error: {0}")]
    StfSyntax(#[from] stf::error::StfError),
    #[error("runtime error: {0}")]
    Runtime(#[from] InterpError),
    #[error("runtime error: {failure} at {span}")]
    Stf {
        failure: Box<StfFailure>,
        span: Span,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum StfFailure {
    #[error("expected {expect} but got {tx}")]
    Mismatch {
        expect: Transmission,
        tx: Transmission,
    },
    #[error("not yet supported: {0}")]
    Unsupported(String),
    #[error("{}{}", remaining_outputs(.txs), remaining_expects(.expects))]
    Remaining {
        txs: Vec<Transmission>,
        expects: Vec<Expectation>,
    },
}

fn remaining_outputs(txs: &[Transmission]) -> String {
    if txs.is_empty() {
        String::new()
    } else {
        format!(
            "[FAIL] Remaining packets to be matched:\n{}",
            txs.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

fn remaining_expects(expects: &[Expectation]) -> String {
    if expects.is_empty() {
        String::new()
    } else {
        format!(
            "[FAIL] Expected packets to be output:\n{}",
            expects
                .iter()
                .map(|expect| expect.tx.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

/// Pipeline values and STF queues belong to one independent input program
pub struct Run {
    pub state: SimState,
    pub tx_output_queue: Vec<Transmission>,
    pub expect_queue: Vec<Expectation>,
    /// Source PASS payloads, in statement order
    pub matches: Vec<Transmission>,
}

impl Run {
    pub fn new(state: SimState) -> Self {
        Self {
            state,
            tx_output_queue: vec![],
            expect_queue: vec![],
            matches: vec![],
        }
    }

    /// Only the first new transmission can consume a pending expectation
    pub fn on_tx_output(&mut self) -> Result<Option<Transmission>, StfFailure> {
        let Some(tx) = self.state.txs.first() else {
            return Ok(None);
        };
        let Some(idx) = self
            .expect_queue
            .iter()
            .position(|expect| expect.tx.port == tx.port)
        else {
            self.tx_output_queue.extend_from_slice(&self.state.txs);
            return Ok(None);
        };
        let expect = &self.expect_queue[idx];
        if !io::matches(tx, expect) {
            return Err(StfFailure::Mismatch {
                expect: expect.tx.clone(),
                tx: tx.clone(),
            });
        }
        let expect = self.expect_queue.remove(idx);
        self.tx_output_queue.extend_from_slice(&self.state.txs[1..]);
        Ok(Some(expect.tx))
    }

    pub fn on_tx_expect(
        &mut self,
        expect: Expectation,
    ) -> Result<Option<Transmission>, StfFailure> {
        let Some(idx) = self
            .tx_output_queue
            .iter()
            .position(|tx| tx.port == expect.tx.port)
        else {
            self.expect_queue.push(expect);
            return Ok(None);
        };
        let tx = &self.tx_output_queue[idx];
        if !io::matches(tx, &expect) {
            return Err(StfFailure::Mismatch {
                expect: expect.tx,
                tx: tx.clone(),
            });
        }
        Ok(Some(self.tx_output_queue.remove(idx)))
    }

    pub fn finish(&self) -> Result<(), StfFailure> {
        if self.tx_output_queue.is_empty() && self.expect_queue.is_empty() {
            Ok(())
        } else {
            Err(StfFailure::Remaining {
                txs: self.tx_output_queue.clone(),
                expects: self.expect_queue.clone(),
            })
        }
    }
}

pub fn init_pipe<Interp, Iface, Arch>(
    runner: &mut Runner<Interp, Iface, Arch>,
    includes: &[PathBuf],
    path: &Path,
) -> Result<Run, Error>
where
    Iface: Interface,
    Arch: Architecture,
    Interp: Interpreter<Iface, Arch, Error = InterpError>,
{
    runner.reset();
    let program = parse::parse_file(runner.arena_mut(), includes, path)?;
    let state = Arch::init_pipe(&mut runner.context(), program)?;
    Ok(Run::new(state))
}

pub fn step<Interp, Iface, Arch>(
    runner: &mut Runner<Interp, Iface, Arch>,
    run: &mut Run,
    stmt: &Phrase<Statement>,
) -> Result<Option<Transmission>, Error>
where
    Iface: Interface,
    Arch: Architecture,
    Interp: Interpreter<Iface, Arch, Error = InterpError>,
{
    run.state.txs.clear();
    let stmt_kind = Arch::transform_stf_stmt(stmt.node.clone());
    let result = step_transformed(runner, run, stmt_kind);
    let tx = result.map_err(|error| match error {
        Error::Runtime(error) => Error::Runtime(error.at_if_missing(&stmt.span)),
        Error::Stf { failure, .. } => Error::Stf {
            failure,
            span: stmt.span.clone(),
        },
        error => error,
    })?;
    if let Some(tx) = &tx {
        run.matches.push(tx.clone());
    }
    Ok(tx)
}

fn step_transformed<Interp, Iface, Arch>(
    runner: &mut Runner<Interp, Iface, Arch>,
    run: &mut Run,
    stmt: Statement,
) -> Result<Option<Transmission>, Error>
where
    Iface: Interface,
    Arch: Architecture,
    Interp: Interpreter<Iface, Arch, Error = InterpError>,
{
    let stf_error = |failure| Error::Stf {
        failure: Box::new(failure),
        span: Span::default(),
    };
    let int = |text: &str| unpack::parse_signed_int(text).map_err(InterpError::from);
    let mut ctx = runner.context();
    let state = &mut run.state;
    match stmt {
        Statement::Packet { port, packet } => {
            let rx = Transmission {
                port: int(&port)?,
                packet: packet.to_ascii_uppercase(),
            };
            Arch::drive_pipe(&mut ctx, state, &rx)?;
            return run.on_tx_output().map_err(stf_error);
        }
        Statement::Expect {
            port,
            packet_expected,
            exact,
        } => {
            let expect = Expectation {
                tx: Transmission {
                    port: int(&port)?,
                    packet: packet_expected.unwrap_or_default().to_ascii_uppercase(),
                },
                exact,
            };
            return run.on_tx_expect(expect).map_err(stf_error);
        }
        Statement::Add {
            table,
            priority,
            matches,
            action,
            ..
        } => {
            let value_name = make::text(
                ctx.arena_mut(),
                escape_name(table.as_str()),
                Span::default(),
            )
            .map_err(InterpError::from)?;
            let value_priority = priority
                .map(|priority| make::int(ctx.arena_mut(), priority.into(), Span::default()))
                .transpose()
                .map_err(InterpError::from)?;
            let value_priority = make::opt(
                ctx.arena_mut(),
                typ::make::opt(typ::make::int()).node.into(),
                value_priority,
                Span::default(),
            )
            .map_err(InterpError::from)?;
            let value_keys = encode_keys(ctx.arena_mut(), &matches)?;
            let value_action = encode_action(ctx.arena_mut(), &action)?;
            state.value_arch = table::add_entry(
                &mut ctx,
                state.value_ctx,
                state.value_arch,
                value_name,
                value_priority,
                value_keys,
                value_action,
            )?;
        }
        Statement::SetDefault { table, action } => {
            let value_name = make::text(ctx.arena_mut(), table.into_string(), Span::default())
                .map_err(InterpError::from)?;
            let value_action = encode_action(ctx.arena_mut(), &action)?;
            state.value_arch = table::add_default_action(
                &mut ctx,
                state.value_ctx,
                state.value_arch,
                value_name,
                value_action,
            )?;
        }
        Statement::MirroringAdd { session, port } => {
            state.value_arch =
                Arch::add_mirror_session(&mut ctx, state.value_arch, int(&session)?, int(&port)?)?;
        }
        Statement::MirroringAddMc { session, group_id } => {
            state.value_arch = Arch::add_mirror_session_mc(
                &mut ctx,
                state.value_arch,
                int(&session)?,
                int(&group_id)?,
            )?;
        }
        Statement::McGroupCreate { group_id } => {
            state.value_arch = Arch::mc_mgrp_create(&mut ctx, state.value_arch, int(&group_id)?)?;
        }
        Statement::McNodeCreate {
            replication_id,
            ports,
        } => {
            let instance = int(&replication_id)?;
            let ports = ports
                .iter()
                .map(|port| int(port))
                .collect::<Result<Vec<_>, _>>()?;
            state.value_arch = Arch::mc_node_create(&mut ctx, state.value_arch, instance, &ports)?;
        }
        Statement::McNodeAssociate { group_id, handle } => {
            state.value_arch = Arch::mc_node_associate(
                &mut ctx,
                state.value_arch,
                int(&group_id)?,
                int(&handle)?,
            )?;
        }
        Statement::RegisterRead { name, index } => {
            state.value_arch =
                Arch::register_read(&mut ctx, state.value_arch, name.as_str(), int(&index)?)?;
        }
        Statement::RegisterWrite { name, index, value } => {
            state.value_arch = Arch::register_write(
                &mut ctx,
                state.value_arch,
                name.as_str(),
                int(&index)?,
                int(&value)?,
            )?;
        }
        Statement::RegisterReset { name } => {
            state.value_arch = Arch::register_reset(&mut ctx, state.value_arch, name.as_str())?;
        }
        Statement::MirroringGet { .. } | Statement::Wait => {}
        stmt => return Err(stf_error(StfFailure::Unsupported(Print::to_string(&stmt)))),
    }
    Ok(None)
}

pub fn run_stf_test<Interp, Iface, Arch>(
    runner: &mut Runner<Interp, Iface, Arch>,
    includes: &[PathBuf],
    path_p4: &Path,
    path_stf: &Path,
) -> Result<Run, Error>
where
    Iface: Interface,
    Arch: Architecture,
    Interp: Interpreter<Iface, Arch, Error = InterpError>,
{
    let mut run = init_pipe(runner, includes, path_p4)?;
    let stmts = stf::parse::parse_file(path_stf)?;
    for stmt in &stmts {
        step(runner, &mut run, stmt)?;
    }
    run.finish().map_err(|failure| Error::Stf {
        failure: Box::new(failure),
        span: Span::default(),
    })?;
    Ok(run)
}

fn typ_named(name: &str) -> typ::Typ {
    typ::make::var(
        crate::phrase!(node: name.to_owned(), span: Span::default()),
        vec![],
    )
}

fn encode_action(arena: &mut ValueArena, action: &Action) -> Result<Value, InterpError> {
    let value_name = make::text(arena, action.name.as_str().to_owned(), Span::default())?;
    let typ_arg = typ_named("tableActionArgumentInterface");
    let mut values_arg = Vec::new();
    for arg in &action.args {
        let value_name = make::text(arena, arg.id.clone(), Span::default())?;
        let int = unpack::parse_signed_int(&arg.num)?;
        let value_int = make::int(arena, int.into(), Span::default())?;
        values_arg.push(make::tuple(
            arena,
            typ_arg.node.clone().into(),
            vec![value_name, value_int],
            Span::default(),
        )?);
    }
    let value_args = make::list(
        arena,
        typ::make::list(typ_arg).node.into(),
        values_arg,
        Span::default(),
    )?;
    Ok(make::tuple(
        arena,
        typ_named("tableActionInterface").node.into(),
        vec![value_name, value_args],
        Span::default(),
    )?)
}

fn encode_keys(arena: &mut ValueArena, matches: &[TableMatch]) -> Result<Value, InterpError> {
    let typ_key = typ_named("tableKeyInterface");
    let mut values_key = Vec::new();
    for key in matches {
        let value_name = make::text(arena, bracket_name(key.name.as_str()), Span::default())?;
        let value_key = match &key.kind {
            MatchKind::Number(num) => {
                let (shape, num) = if let Some(num) = num.strip_prefix("0x") {
                    ("_HEX text", num)
                } else if let Some(num) = num.strip_prefix("0b") {
                    ("_BIN text", num)
                } else {
                    ("_DEC text", num.as_str())
                };
                let value_num = make::text(arena, num.to_owned(), Span::default())?;
                make::case_shaped_(
                    arena,
                    shape,
                    vec![value_num],
                    "tableKeyValueInterface",
                    Span::default(),
                )?
            }
            MatchKind::Slash(prefix, mask) => {
                let value_prefix = make::text(arena, prefix.clone(), Span::default())?;
                let mask = unpack::parse_signed_int(mask)?;
                let nat = crate::lang::xl::num::Natural::try_from(num_bigint::BigInt::from(mask))?;
                let value_mask = make::nat(arena, nat, Span::default())?;
                make::case_shaped_(
                    arena,
                    "text _SLASH nat",
                    vec![value_prefix, value_mask],
                    "tableKeyValueInterface",
                    Span::default(),
                )?
            }
        };
        values_key.push(make::tuple(
            arena,
            typ_key.node.clone().into(),
            vec![value_name, value_key],
            Span::default(),
        )?);
    }
    Ok(make::list(
        arena,
        typ::make::list(typ_key).node.into(),
        values_key,
        Span::default(),
    )?)
}

fn escape_name(text: &str) -> String {
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

fn bracket_name(name: &str) -> String {
    let mut text = String::new();
    let mut chars = name.chars().peekable();
    while let Some(char) = chars.next() {
        if char == '$' && chars.peek().is_some_and(char::is_ascii_digit) {
            text.push('[');
            while chars.peek().is_some_and(char::is_ascii_digit) {
                text.push(chars.next().expect("peeked digit"));
            }
            text.push(']');
        } else {
            text.push(char);
        }
    }
    text
}
