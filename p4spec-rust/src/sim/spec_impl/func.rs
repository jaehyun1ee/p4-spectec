use crate::{
    lang::{
        common::source::Span,
        data::value::{Value, make},
    },
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
};

pub fn find_var_value_t<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_cursor: &Value,
    value_ctx: &Value,
    name: &str,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value_name = make::text(ctx.arena_mut(), name.to_owned(), Span::default())
        .map_err(ExternError::from)
        .map_err(Interp::Error::from)?;
    let value_name = make::case_shaped_(
        ctx.arena_mut(),
        "_BARE nameIR",
        vec![value_name],
        "prefixedNameIR",
        Span::default(),
    )
    .map_err(ExternError::from)
    .map_err(Interp::Error::from)?;
    ctx.call_func(
        "find_var_value_t",
        &[],
        &[value_name, *value_cursor, *value_ctx],
    )
}

pub fn find_var_value_t_local<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: &Value,
    name: &str,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value_cursor = make::case_shaped_(
        ctx.arena_mut(),
        "LOCAL",
        Vec::new(),
        "cursor",
        Span::default(),
    )
    .map_err(ExternError::from)
    .map_err(Interp::Error::from)?;
    find_var_value_t(ctx, &value_cursor, value_ctx, name)
}
