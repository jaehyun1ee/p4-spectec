use crate::{
    lang::data::value::{Value, get},
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CallResult {
    pub value_ctx: Value,
    pub value_arch: Value,
    pub value_call_result: Value,
}

pub fn lvalue_write_var_local<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: Value,
    value_arch: Value,
    name: &str,
    value: Value,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value_cursor = super::func::local_cursor(ctx.arena_mut())?;
    let value_name = super::func::bare_name(ctx.arena_mut(), name)?;
    let values = ctx.call_rel(
        "Lvalue_write",
        &[value_cursor, value_ctx, value_arch, value_name, value],
    )?;
    Ok(*get::one(&values).map_err(ExternError::from)?)
}
