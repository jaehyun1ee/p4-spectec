use crate::{
    lang::{
        common::source::Span,
        data::value::{Value, make},
    },
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
};

use super::super::spec_impl::{func, pack, rel::CallResult, unpack};

pub fn static_assert<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: &Value,
    has_message: bool,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value_check = func::find_var_value_t_local(ctx, value_ctx, "check")?;
    let value_message = if has_message {
        Some(func::find_var_value_t_local(ctx, value_ctx, "message")?)
    } else {
        None
    };
    let check = unpack::p4_bool(ctx.arena(), &value_check).map_err(Interp::Error::from)?;
    if check {
        return Ok(value_check);
    }
    let message = match value_message {
        Some(value) => unpack::p4_string(ctx.arena(), &value).map_err(Interp::Error::from)?,
        None => "static_assert failed".to_owned(),
    };
    Err(crate::runner::ExternError::Failure(message).into())
}

pub fn verify<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: Value,
    value_arch: Value,
) -> Result<CallResult, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value_check = func::find_var_e_local(ctx, value_ctx, "check")?;
    let value_signal = func::find_var_e_local(ctx, value_ctx, "toSignal")?;
    let check = unpack::p4_bool(ctx.arena(), &value_check)?;
    let value_call_result = if check {
        pack::return_result(ctx.arena_mut(), None)?
    } else {
        make::case_shaped_(
            ctx.arena_mut(),
            "REJECT errorValue",
            vec![value_signal],
            "rejectResult",
            Span::default(),
        )
        .map_err(ExternError::from)?
    };
    Ok(CallResult {
        value_ctx,
        value_arch,
        value_call_result,
    })
}
