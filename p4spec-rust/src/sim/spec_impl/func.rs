use crate::{
    lang::{
        common::source::Span,
        data::value::{Value, make},
    },
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
};

pub fn find_var_value_t<S, I, E>(
    context: &mut RunnerContext<'_, S, I, E>,
    value_cursor: &Value,
    value_ctx: &Value,
    name: &str,
) -> Result<Value, S::Error>
where
    I: Interface,
    E: Extern,
    S: Interpreter<I, E>,
{
    let value_name = make::text(context.arena_mut(), name.to_owned(), Span::default())
        .map_err(ExternError::from)
        .map_err(S::Error::from)?;
    let value_name = make::case_shaped_(
        context.arena_mut(),
        "_BARE nameIR",
        vec![value_name],
        "prefixedNameIR",
        Span::default(),
    )
    .map_err(ExternError::from)
    .map_err(S::Error::from)?;
    context.call_func(
        "find_var_value_t",
        &[],
        &[value_name, *value_cursor, *value_ctx],
    )
}

pub fn find_var_value_t_local<S, I, E>(
    context: &mut RunnerContext<'_, S, I, E>,
    value_ctx: &Value,
    name: &str,
) -> Result<Value, S::Error>
where
    I: Interface,
    E: Extern,
    S: Interpreter<I, E>,
{
    let value_cursor = make::case_shaped_(
        context.arena_mut(),
        "LOCAL",
        Vec::new(),
        "cursor",
        Span::default(),
    )
    .map_err(ExternError::from)
    .map_err(S::Error::from)?;
    find_var_value_t(context, &value_cursor, value_ctx, name)
}
