//! Helpers for invoking relations taking a program in the spec

use crate::{
    lang::data::value::{Value, get},
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
};

// == eBPF

pub fn ebpf_init<Interp, Iface, Ext>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
    program: Value,
) -> Result<(Value, Value), Interp::Error>
where
    Iface: Interface,
    Ext: Extern,
    Interp: Interpreter<Iface, Ext>,
{
    let values = ctx.call_program("EBPF_init", program)?;
    let (value_ctx, value_arch) = get::two(&values).map_err(ExternError::from)?;
    Ok((*value_ctx, *value_arch))
}

// == PSA

pub fn psa_init<Interp, Iface, Ext>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
    program: Value,
) -> Result<(Value, Value), Interp::Error>
where
    Iface: Interface,
    Ext: Extern,
    Interp: Interpreter<Iface, Ext>,
{
    let values = ctx.call_program("PSA_init", program)?;
    let (value_ctx, value_arch) = get::two(&values).map_err(ExternError::from)?;
    Ok((*value_ctx, *value_arch))
}

// == v1model

pub fn v1model_init<Interp, Iface, Ext>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
    program: Value,
) -> Result<(Value, Value), Interp::Error>
where
    Iface: Interface,
    Ext: Extern,
    Interp: Interpreter<Iface, Ext>,
{
    let values = ctx.call_program("V1Model_init", program)?;
    let (value_ctx, value_arch) = get::two(&values).map_err(ExternError::from)?;
    Ok((*value_ctx, *value_arch))
}
