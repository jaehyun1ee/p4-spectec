use crate::{
    lang::data::value::Value,
    runner::{Extern, Interface, Interpreter, RunnerContext},
};

use super::super::spec_impl::{func, unpack};

pub fn static_assert<S, I, E>(
    ctx: &mut RunnerContext<'_, S, I, E>,
    value_ctx: &Value,
    has_message: bool,
) -> Result<Value, S::Error>
where
    I: Interface,
    E: Extern,
    S: Interpreter<I, E>,
{
    let value_check = func::find_var_value_t_local(ctx, value_ctx, "check")?;
    let value_message = if has_message {
        Some(func::find_var_value_t_local(ctx, value_ctx, "message")?)
    } else {
        None
    };
    let check = unpack::p4_bool(ctx.arena(), &value_check).map_err(S::Error::from)?;
    if check {
        return Ok(value_check);
    }
    let message = match value_message {
        Some(value) => unpack::p4_string(ctx.arena(), &value).map_err(S::Error::from)?,
        None => "static_assert failed".to_owned(),
    };
    Err(crate::runner::ExternError::Failure(message).into())
}
