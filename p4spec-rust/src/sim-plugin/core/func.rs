use crate::{
    lang::{
        common::source::Span,
        data::{
            typ,
            value::{Value, make},
        },
    },
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
};

use super::super::spec_impl::{func, unpack};

/// Evaluates a boolean expression at compilation time and stops compilation
/// with the supplied message when the expression is false
///
/// The boolean result can initialize a global constant, for example:
/// ```text
/// const bool _check = static_assert(
///     V1MODEL_VERSION > 20180000,
///     "Expected a v1 model version >= 20180000");
/// ```
///
/// The overload without a message uses the default failure message:
/// ```text
/// extern bool static_assert(bool check, string message);
/// extern bool static_assert(bool check);
/// ```
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

/// Checks a predicate in the parser, leaving execution unchanged when true
///
/// A false predicate sets the parser error to `toSignal` and transitions to
/// the `reject` state:
/// ```text
/// extern void verify(in bool check, in error toSignal);
/// ```
pub fn verify<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: Value,
    value_arch: Value,
) -> Result<(Value, Value, Value), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value_check = func::find_var_e_local(ctx, value_ctx, "check")?;
    let value_signal = func::find_var_e_local(ctx, value_ctx, "toSignal")?;
    let check = unpack::p4_bool(ctx.arena(), &value_check)?;
    let value_call_result = if check {
        let typ = typ::make::opt(typ::make::var(
            crate::phrase!(node: "value".to_owned(), span: Span::default()),
            Vec::new(),
        ));
        let value_opt = make::opt(ctx.arena_mut(), typ.node.into(), None, Span::default())
            .map_err(ExternError::from)?;
        make::case_shaped_(
            ctx.arena_mut(),
            "RETURN value?",
            vec![value_opt],
            "returnResult",
            Span::default(),
        )
        .map_err(ExternError::from)?
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
    Ok((value_ctx, value_arch, value_call_result))
}
