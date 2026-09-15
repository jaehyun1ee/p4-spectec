//! Transforms and executes STF statements against one runner and per-run state

use super::{
    arch::Architecture,
    io::{self, Expectation, Rx, Tx},
    state::SimState,
    table,
};
use crate::{
    interface::p4::{error::P4Error, parse, unparse::P4Unparser},
    interp::al::error::Error as InterpError,
    lang::{
        common::source::{Phrase, Span},
        data::{
            typ,
            value::{Value, ValueArena, make},
        },
        traits::print::Print,
    },
    runner::{ExternError, Interface, Interpreter, Runner, RunnerContext},
    stf::{
        self,
        ast::{Action, MatchKind, Name, Statement, TableMatch},
    },
};
use num_bigint::BigInt;
use std::path::{Path, PathBuf};

// == Errors

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("syntax error: {0}")]
    P4Syntax(#[from] P4Error),
    #[error("runtime error: {0}")]
    StfSyntax(#[from] stf::error::StfError),
    #[error("runtime error: {0}")]
    Runtime(#[from] InterpError),
    #[error("runtime error: {failure} at {span}")]
    Stf { failure: Box<StfFailure>, span: Span },
}

#[derive(Debug, thiserror::Error)]
pub enum StfFailure {
    #[error("expected {expect} but got {tx}")]
    Mismatch { expect: Tx, tx: Tx },
    #[error("not yet supported: {0}")]
    Unsupported(String),
    #[error("{}{}", remaining_outputs(.txs), remaining_expects(.expects))]
    Remaining { txs: Vec<Tx>, expects: Vec<Expectation> },
}

fn remaining_outputs(txs: &[Tx]) -> String {
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

// == Helpers

/// Parses an optionally signed integer with a `0x`, `0o` or `0b` radix prefix
fn parse_int<Int: strtoint::StrToInt>(text: &str) -> Result<Int, InterpError> {
    strtoint::strtoint(&text.to_ascii_lowercase())
        .map_err(|_| ExternError::Failure(format!("invalid integer: {text}")).into())
}

fn convert_dollar_to_brackets(name: &str) -> String {
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

// == Run state

/// Pipeline values and STF queues belong to one independent input program
pub struct Run {
    pub state: SimState,
    pub tx_output_queue: Vec<Tx>,
    pub expect_queue: Vec<Expectation>,
    /// Source PASS payloads, in statement order
    pub matches: Vec<Tx>,
}

impl Run {
    pub fn new(state: SimState) -> Self {
        Self { state, tx_output_queue: vec![], expect_queue: vec![], matches: vec![] }
    }

    /// Only the first new transmission can consume a pending expectation
    pub fn on_tx_output(&mut self) -> Result<Option<Tx>, StfFailure> {
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
            return Err(StfFailure::Mismatch { expect: expect.tx.clone(), tx: tx.clone() });
        }
        let expect = self.expect_queue.remove(idx);
        self.tx_output_queue.extend_from_slice(&self.state.txs[1..]);
        Ok(Some(expect.tx))
    }

    pub fn on_tx_expect(&mut self, expect: Expectation) -> Result<Option<Tx>, StfFailure> {
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
            return Err(StfFailure::Mismatch { expect: expect.tx, tx: tx.clone() });
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

// == Pipeline initialization

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

// == STF statements

pub fn run_stf_stmt<Interp, Iface, Arch>(
    runner: &mut Runner<Interp, Iface, Arch>,
    run: &mut Run,
    stmt: &Phrase<Statement>,
) -> Result<Option<Tx>, Error>
where
    Iface: Interface,
    Arch: Architecture,
    Interp: Interpreter<Iface, Arch, Error = InterpError>,
{
    run.state.txs.clear();
    let stmt_kind = Arch::transform_stf_stmt(stmt.node.clone());
    let mut ctx = runner.context();
    let result = match stmt_kind {
        Statement::Packet { port, packet } => run_stf_packet_stmt(&mut ctx, run, port, packet),
        Statement::Expect { port, packet_expected, exact } => {
            run_stf_expect_stmt(run, port, packet_expected, exact)
        }
        Statement::Add { table, priority, matches, action, .. } => {
            run_stf_add_stmt(&mut ctx, &mut run.state, table, priority, matches, action)
        }
        Statement::SetDefault { table, action } => {
            run_stf_set_default_stmt(&mut ctx, &mut run.state, table, action)
        }
        Statement::MirroringAdd { session, port } => {
            run_stf_mirroring_add_stmt(&mut ctx, &mut run.state, session, port)
        }
        Statement::MirroringAddMc { session, group_id } => {
            run_stf_mirroring_add_mc_stmt(&mut ctx, &mut run.state, session, group_id)
        }
        Statement::McGroupCreate { group_id } => {
            run_stf_mc_group_create_stmt(&mut ctx, &mut run.state, group_id)
        }
        Statement::McNodeCreate { replication_id, ports } => {
            run_stf_mc_node_create_stmt(&mut ctx, &mut run.state, replication_id, ports)
        }
        Statement::McNodeAssociate { group_id, handle } => {
            run_stf_mc_node_associate_stmt(&mut ctx, &mut run.state, group_id, handle)
        }
        Statement::RegisterRead { name, index } => {
            run_stf_register_read_stmt(&mut ctx, &mut run.state, name, index)
        }
        Statement::RegisterWrite { name, index, value } => {
            run_stf_register_write_stmt(&mut ctx, &mut run.state, name, index, value)
        }
        Statement::RegisterReset { name } => {
            run_stf_register_reset_stmt(&mut ctx, &mut run.state, name)
        }
        Statement::MirroringGet { .. } | Statement::Wait => Ok(None),
        stmt => Err(Error::Stf {
            failure: Box::new(StfFailure::Unsupported(Print::to_string(&stmt))),
            span: Span::default(),
        }),
    };
    let tx = result.map_err(|error| match error {
        Error::Runtime(error) => Error::Runtime(error.at_if_missing(&stmt.span)),
        Error::Stf { failure, .. } => Error::Stf { failure, span: stmt.span.clone() },
        error => error,
    })?;
    if let Some(tx) = &tx {
        run.matches.push(tx.clone());
    }
    Ok(tx)
}

// - Packet I/O

fn run_stf_packet_stmt<Interp, Iface, Arch>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Arch>,
    run: &mut Run,
    port: String,
    packet: String,
) -> Result<Option<Tx>, Error>
where
    Iface: Interface,
    Arch: Architecture,
    Interp: Interpreter<Iface, Arch, Error = InterpError>,
{
    let rx = Rx { port: parse_int::<usize>(&port)?, packet: packet.to_ascii_uppercase() };
    Arch::drive_pipe(ctx, &mut run.state, &rx)?;
    run.on_tx_output()
        .map_err(|failure| Error::Stf { failure: Box::new(failure), span: Span::default() })
}

fn run_stf_expect_stmt(
    run: &mut Run,
    port: String,
    packet_expected: Option<String>,
    exact: bool,
) -> Result<Option<Tx>, Error> {
    let expect = Expectation {
        tx: Tx {
            port: parse_int::<usize>(&port)?,
            packet: packet_expected.unwrap_or_default().to_ascii_uppercase(),
        },
        exact,
    };
    run.on_tx_expect(expect)
        .map_err(|failure| Error::Stf { failure: Box::new(failure), span: Span::default() })
}

// - Match-action table updates

fn encode_table_keys(arena: &mut ValueArena, matches: &[TableMatch]) -> Result<Value, InterpError> {
    let typ_key = typ::make::var(
        crate::phrase!(node: "tableKeyInterface".to_owned(), span: Span::default()),
        vec![],
    );
    let mut values_key = Vec::new();
    for key in matches {
        let value_name =
            make::text(arena, convert_dollar_to_brackets(key.name.as_str()), Span::default())?;
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
                make::case_shaped! {
                    arena: arena,
                    shape: shape,
                    args: vec![value_num],
                    typ: "tableKeyValueInterface",
                    span: Span::default(),
                }?
            }
            MatchKind::Slash(prefix, mask) => {
                let value_prefix = make::text(arena, prefix.clone(), Span::default())?;
                let mask = BigInt::from(parse_int::<i128>(mask)?);
                let nat = crate::lang::xl::num::Natural::try_from(mask)?;
                let value_mask = make::nat(arena, nat, Span::default())?;
                make::case_shaped! {
                    arena: arena,
                    shape: "text _SLASH nat",
                    args: vec![value_prefix, value_mask],
                    typ: "tableKeyValueInterface",
                    span: Span::default(),
                }?
            }
        };
        values_key.push(make::tuple(
            arena,
            typ_key.node.clone().into(),
            vec![value_name, value_key],
            Span::default(),
        )?);
    }
    Ok(make::list(arena, typ::make::list(typ_key).node.into(), values_key, Span::default())?)
}

fn run_stf_add_stmt<Interp, Iface, Arch>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Arch>,
    state: &mut SimState,
    table: Name,
    priority: Option<i64>,
    matches: Vec<TableMatch>,
    action: Action,
) -> Result<Option<Tx>, Error>
where
    Iface: Interface,
    Arch: Architecture,
    Interp: Interpreter<Iface, Arch, Error = InterpError>,
{
    // Add names use the same escaped spelling as P4 annotation names.
    let name = P4Unparser::escape_text(&table.into_string());
    let value_name =
        make::text(ctx.arena_mut(), name, Span::default()).map_err(InterpError::from)?;
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
    let value_keys = encode_table_keys(ctx.arena_mut(), &matches)?;
    let value_action = encode_table_action(ctx.arena_mut(), &action)?;
    state.value_arch = table::add_entry(
        ctx,
        state.value_ctx,
        state.value_arch,
        value_name,
        value_priority,
        value_keys,
        value_action,
    )?;
    Ok(None)
}

fn encode_table_action(arena: &mut ValueArena, action: &Action) -> Result<Value, InterpError> {
    let value_name = make::text(arena, action.name.as_str().to_owned(), Span::default())?;
    let typ_arg = typ::make::var(
        crate::phrase!(node: "tableActionArgumentInterface".to_owned(), span: Span::default()),
        vec![],
    );
    let mut values_arg = Vec::new();
    for arg in &action.args {
        let value_name = make::text(arena, arg.id.clone(), Span::default())?;
        let int = BigInt::from(parse_int::<i128>(&arg.num)?);
        let value_int = make::int(arena, int, Span::default())?;
        values_arg.push(make::tuple(
            arena,
            typ_arg.node.clone().into(),
            vec![value_name, value_int],
            Span::default(),
        )?);
    }
    let value_args =
        make::list(arena, typ::make::list(typ_arg).node.into(), values_arg, Span::default())?;
    Ok(make::tuple(
        arena,
        typ::make::var(
            crate::phrase!(node: "tableActionInterface".to_owned(), span: Span::default()),
            vec![],
        )
        .node
        .into(),
        vec![value_name, value_args],
        Span::default(),
    )?)
}

fn run_stf_set_default_stmt<Interp, Iface, Arch>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Arch>,
    state: &mut SimState,
    table: Name,
    action: Action,
) -> Result<Option<Tx>, Error>
where
    Iface: Interface,
    Arch: Architecture,
    Interp: Interpreter<Iface, Arch, Error = InterpError>,
{
    let value_name = make::text(ctx.arena_mut(), table.into_string(), Span::default())
        .map_err(InterpError::from)?;
    let value_action = encode_table_action(ctx.arena_mut(), &action)?;
    state.value_arch = table::add_default_action(
        ctx,
        state.value_ctx,
        state.value_arch,
        value_name,
        value_action,
    )?;
    Ok(None)
}

// - Mirror session updates

fn run_stf_mirroring_add_stmt<Interp, Iface, Arch>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Arch>,
    state: &mut SimState,
    session: String,
    port: String,
) -> Result<Option<Tx>, Error>
where
    Iface: Interface,
    Arch: Architecture,
    Interp: Interpreter<Iface, Arch, Error = InterpError>,
{
    state.value_arch = Arch::add_mirror_session(
        ctx,
        state.value_arch,
        parse_int::<usize>(&session)?,
        parse_int::<usize>(&port)?,
    )?;
    Ok(None)
}

fn run_stf_mirroring_add_mc_stmt<Interp, Iface, Arch>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Arch>,
    state: &mut SimState,
    session: String,
    id_group: String,
) -> Result<Option<Tx>, Error>
where
    Iface: Interface,
    Arch: Architecture,
    Interp: Interpreter<Iface, Arch, Error = InterpError>,
{
    state.value_arch = Arch::add_mirror_session_mc(
        ctx,
        state.value_arch,
        parse_int::<usize>(&session)?,
        parse_int::<usize>(&id_group)?,
    )?;
    Ok(None)
}

// - Multicast group updates

fn run_stf_mc_group_create_stmt<Interp, Iface, Arch>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Arch>,
    state: &mut SimState,
    id_group: String,
) -> Result<Option<Tx>, Error>
where
    Iface: Interface,
    Arch: Architecture,
    Interp: Interpreter<Iface, Arch, Error = InterpError>,
{
    state.value_arch = Arch::mc_mgrp_create(ctx, state.value_arch, parse_int::<usize>(&id_group)?)?;
    Ok(None)
}

fn run_stf_mc_node_create_stmt<Interp, Iface, Arch>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Arch>,
    state: &mut SimState,
    id_replication: String,
    ports: Vec<String>,
) -> Result<Option<Tx>, Error>
where
    Iface: Interface,
    Arch: Architecture,
    Interp: Interpreter<Iface, Arch, Error = InterpError>,
{
    let instance = parse_int::<usize>(&id_replication)?;
    let ports = ports
        .iter()
        .map(|port| parse_int::<usize>(port))
        .collect::<Result<Vec<_>, _>>()?;
    state.value_arch = Arch::mc_node_create(ctx, state.value_arch, instance, &ports)?;
    Ok(None)
}

fn run_stf_mc_node_associate_stmt<Interp, Iface, Arch>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Arch>,
    state: &mut SimState,
    id_group: String,
    handle: String,
) -> Result<Option<Tx>, Error>
where
    Iface: Interface,
    Arch: Architecture,
    Interp: Interpreter<Iface, Arch, Error = InterpError>,
{
    state.value_arch = Arch::mc_node_associate(
        ctx,
        state.value_arch,
        parse_int::<usize>(&id_group)?,
        parse_int::<usize>(&handle)?,
    )?;
    Ok(None)
}

// - Register updates

fn run_stf_register_read_stmt<Interp, Iface, Arch>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Arch>,
    state: &mut SimState,
    name: Name,
    idx: String,
) -> Result<Option<Tx>, Error>
where
    Iface: Interface,
    Arch: Architecture,
    Interp: Interpreter<Iface, Arch, Error = InterpError>,
{
    state.value_arch =
        Arch::register_read(ctx, state.value_arch, name.as_str(), parse_int::<usize>(&idx)?)?;
    Ok(None)
}

fn run_stf_register_write_stmt<Interp, Iface, Arch>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Arch>,
    state: &mut SimState,
    name: Name,
    idx: String,
    value: String,
) -> Result<Option<Tx>, Error>
where
    Iface: Interface,
    Arch: Architecture,
    Interp: Interpreter<Iface, Arch, Error = InterpError>,
{
    state.value_arch = Arch::register_write(
        ctx,
        state.value_arch,
        name.as_str(),
        parse_int::<usize>(&idx)?,
        BigInt::from(parse_int::<i128>(&value)?),
    )?;
    Ok(None)
}

fn run_stf_register_reset_stmt<Interp, Iface, Arch>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Arch>,
    state: &mut SimState,
    name: Name,
) -> Result<Option<Tx>, Error>
where
    Iface: Interface,
    Arch: Architecture,
    Interp: Interpreter<Iface, Arch, Error = InterpError>,
{
    state.value_arch = Arch::register_reset(ctx, state.value_arch, name.as_str())?;
    Ok(None)
}

// == STF tests

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
        run_stf_stmt(runner, &mut run, stmt)?;
    }
    run.finish()
        .map_err(|failure| Error::Stf { failure: Box::new(failure), span: Span::default() })?;
    Ok(run)
}
