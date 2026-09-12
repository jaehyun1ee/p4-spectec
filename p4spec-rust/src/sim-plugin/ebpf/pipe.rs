use crate::{
    lang::{
        data::value::{Value, ValueArena, get},
        il::ast::Typ,
    },
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
    stf::ast::{Name, Statement},
    util::json::json,
};
use serde::{Deserialize, Serialize};

use super::{
    super::{
        core::{func as core_func, object::PacketIn},
        externs::{self as external, FuncName, RelName},
        io::Transmission,
        spec_impl::{func, pgm, rel, unpack},
        state::{SimState, install_result},
    },
    object::CounterArray,
};

pub struct Ebpf;

// Extern objects
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExternObject {
    PacketIn(PacketIn),
    CounterArray(CounterArray),
}

impl ExternObject {
    pub fn from_value(arena: &ValueArena, value: &Value) -> Result<Self, ExternError> {
        let json = get::external(arena, value)?;
        let object: Self = serde_json::from_value(json.clone())
            .map_err(|error| ExternError::Failure(error.to_string()))?;
        if let Self::PacketIn(pkt) = &object {
            pkt.payload()?;
        }
        Ok(object)
    }

    pub fn to_value(&self, arena: &mut ValueArena) -> Result<Value, ExternError> {
        let json =
            serde_json::to_value(self).map_err(|error| ExternError::Failure(error.to_string()))?;
        external::state_value(arena, "objectState", json)
    }
}

// Extern calls

impl Extern for Ebpf {
    fn eval_func<Interp, Iface>(
        &self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        name: &str,
        _targs: &[Typ],
        values: &[Value],
    ) -> Result<(Value, bool), Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        let value = match external::func_name(name)? {
            FuncName::InitArch => external::state_value(ctx.arena_mut(), "archState", json::Null)?,
            FuncName::InitObject => {
                let [value_name, _value_targs, value_ids, value_args] = values else {
                    return Err(ExternError::Failure(
                        "unexpected number of arguments to extern init".to_owned(),
                    )
                    .into());
                };
                let name = get::text(ctx.arena(), value_name).map_err(ExternError::from)?;
                if name == "CounterArray" {
                    let counter = CounterArray::init(ctx.arena(), *value_ids, *value_args)?;
                    ExternObject::CounterArray(counter).to_value(ctx.arena_mut())?
                } else {
                    external::state_value(ctx.arena_mut(), "objectState", json::Null)?
                }
            }
        };
        Ok((value, false))
    }

    fn eval_rel<Interp, Iface>(
        &self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        name: &str,
        values: &[Value],
    ) -> Result<(Vec<Value>, bool), Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        let values = match external::rel_name(name)? {
            RelName::FuncLctk => external::eval_func_lctk(ctx, values)?,
            RelName::Func => eval_func(ctx, values)?,
            RelName::Method => eval_method(ctx, values)?,
        };
        Ok((values, false))
    }

    fn clear(&mut self) {}
}

fn eval_func<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Ebpf>,
    values: &[Value],
) -> Result<Vec<Value>, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Ebpf>,
{
    let [value_ctx, value_arch, value_name, value_names] = values else {
        return Err(ExternError::Failure(
            "unexpected number of arguments to extern function call".to_owned(),
        )
        .into());
    };
    let name = get::text(ctx.arena(), value_name)
        .map_err(ExternError::from)?
        .to_owned();
    let names = external::param_names(ctx.arena(), *value_names)?;
    let result = if name == "verify" && names == ["check", "toSignal"] {
        core_func::verify(ctx, *value_ctx, *value_arch)?
    } else {
        return Err(ExternError::Failure(format!(
            "unsupported extern function call: {name}({})",
            names.join(", ")
        ))
        .into());
    };
    Ok(vec![
        result.value_ctx,
        result.value_arch,
        result.value_call_result,
    ])
}

fn eval_method<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Ebpf>,
    values: &[Value],
) -> Result<Vec<Value>, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Ebpf>,
{
    let [value_ctx, value_arch, value_id, value_name, value_names] = values else {
        return Err(ExternError::Failure(
            "unexpected number of arguments to extern method call".to_owned(),
        )
        .into());
    };
    let value_state = func::find_object_state_e(ctx, *value_arch, *value_id)?;
    let object = ExternObject::from_value(ctx.arena(), &value_state)?;
    let name = get::text(ctx.arena(), value_name)
        .map_err(ExternError::from)?
        .to_owned();
    let names = external::param_names(ctx.arena(), *value_names)?;
    let (object, result) = match object {
        ExternObject::PacketIn(pkt) => {
            let output = match (
                name.as_str(),
                names
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .as_slice(),
            ) {
                ("extract", ["hdr"]) => pkt.extract(ctx, *value_ctx, *value_arch)?,
                ("extract", ["variableSizeHeader", "variableFieldSizeInBits"]) => {
                    pkt.extract_varsize(ctx, *value_ctx, *value_arch)?
                }
                ("lookahead", []) => pkt.lookahead(ctx, *value_ctx, *value_arch)?,
                ("advance", ["sizeInBits"]) => pkt.advance(ctx, *value_ctx, *value_arch)?,
                ("length", []) => pkt.length(ctx, *value_ctx, *value_arch)?,
                _ => return Err(unsupported_method(ctx.arena(), *value_id, &name, &names)?.into()),
            };
            (ExternObject::PacketIn(output.pkt), output.result)
        }
        ExternObject::CounterArray(counter) => {
            let output = match (
                name.as_str(),
                names
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .as_slice(),
            ) {
                ("increment", ["index"]) => counter.increment(ctx, *value_ctx, *value_arch)?,
                ("add", ["index", "value"]) => counter.add(ctx, *value_ctx, *value_arch)?,
                _ => return Err(unsupported_method(ctx.arena(), *value_id, &name, &names)?.into()),
            };
            (ExternObject::CounterArray(output.counter), output.result)
        }
    };
    let value_state = object.to_value(ctx.arena_mut())?;
    let value_arch = func::update_object_state_e(ctx, result.value_arch, *value_id, value_state)?;
    Ok(vec![result.value_ctx, value_arch, result.value_call_result])
}

fn unsupported_method(
    arena: &ValueArena,
    value_id: Value,
    name: &str,
    names: &[String],
) -> Result<ExternError, ExternError> {
    let ids = external::param_names(arena, value_id)?;
    Ok(ExternError::Failure(format!(
        "unsupported extern method call: {}.{name}({})",
        ids.join("."),
        names.join(", ")
    )))
}

/// STF transformation
pub fn transform_stf_stmt(mut stmt: Statement) -> Statement {
    fn transform_name(name: Name) -> Name {
        name.replace_substring(&["pipe_c1_"], "main.filt.c1.")
            .replace_substring(&["pipe_"], "main.filt.")
            .replace_substring(&["pipe"], "main.filt")
    }
    match &mut stmt {
        Statement::Add { table, action, .. } | Statement::SetDefault { table, action } => {
            *table = transform_name(table.clone());
            *action = action
                .clone()
                .replace_substring(&["pipe_c1_"], "main.filt.c1.")
                .replace_substring(&["pipe_"], "main.filt.")
                .replace_substring(&["_NoAction"], "NoAction")
                .into_unqualified();
        }
        _ => {}
    }
    stmt
}

/// Pipeline initializer
pub fn init_pipe<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    program: Value,
) -> Result<SimState, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let result = pgm::ebpf_init(ctx, program)?;
    Ok(SimState {
        value_ctx: result.value_ctx,
        value_arch: result.value_arch,
        txs: vec![],
    })
}

/// Pipeline driver
pub fn drive_pipe<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
    rx: &Transmission,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    state.txs.clear();
    // Setup packet_in extern
    let pkt = ExternObject::PacketIn(PacketIn::init(&rx.packet)?);
    let value_packet = pkt.to_value(ctx.arena_mut())?;
    let result = rel::ebpf_init_packet_in(ctx, state.value_ctx, state.value_arch, value_packet)?;
    install_result!(state, result);
    // Setup global variables
    state.value_ctx = rel::ebpf_init_globals(ctx, state.value_ctx, state.value_arch)?;
    // Parse block
    let result = rel::ebpf_parse(ctx, state.value_ctx, state.value_arch)?;
    install_result!(state, result);
    let rejected = get::matches! { ctx.arena(), &result.value_call_result,
        "REJECT errorValue" => |_values| true,
        _ => false,
    };
    if rejected {
        return Ok(());
    }
    // Filter block
    let result = rel::ebpf_filter(ctx, state.value_ctx, state.value_arch)?;
    install_result!(state, result);
    // Check if packet is accepted
    let value_accept =
        rel::lvalue_read_var_global(ctx, state.value_ctx, state.value_arch, "accept")?;
    if unpack::p4_bool(ctx.arena(), &value_accept)? {
        state.txs.push(Transmission {
            port: rx.port,
            packet: rx.packet.clone(),
        });
    }
    Ok(())
}
