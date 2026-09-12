//! Helpers for invoking relations taking a program in the spec

use super::rel::StateResult;
use crate::{
    lang::data::value::{Value, get},
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
};

pub fn ebpf_init<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    program: Value,
) -> Result<StateResult, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let values = ctx.call_program("EBPF_init", program)?;
    let (value_ctx, value_arch) = get::two(&values).map_err(ExternError::from)?;
    Ok(StateResult {
        value_ctx: *value_ctx,
        value_arch: *value_arch,
    })
}

pub fn psa_init<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    program: Value,
) -> Result<StateResult, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let values = ctx.call_program("PSA_init", program)?;
    let (value_ctx, value_arch) = get::two(&values).map_err(ExternError::from)?;
    Ok(StateResult {
        value_ctx: *value_ctx,
        value_arch: *value_arch,
    })
}
