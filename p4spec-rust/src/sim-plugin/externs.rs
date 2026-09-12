use super::core;
use crate::{
    lang::{
        common::source::Span,
        data::{
            typ,
            value::{Value, ValueArena, get, make},
        },
    },
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
    util::json::json,
};

pub(crate) enum FuncName {
    InitObject,
    InitArch,
}
pub(crate) enum RelName {
    FuncLctk,
    Func,
    Method,
}

pub(crate) fn func_name(name: &str) -> Result<FuncName, ExternError> {
    match name {
        "init_objectState" => Ok(FuncName::InitObject),
        "init_archState" => Ok(FuncName::InitArch),
        _ => Err(ExternError::Failure(format!(
            "unimplemented extern function: {name}"
        ))),
    }
}

pub(crate) fn rel_name(name: &str) -> Result<RelName, ExternError> {
    match name {
        "ExternFunctionCall_eval_lctk" => Ok(RelName::FuncLctk),
        "ExternFunctionCall_eval" => Ok(RelName::Func),
        "ExternMethodCall_eval" => Ok(RelName::Method),
        _ => Err(ExternError::Failure(format!(
            "unimplemented extern relation: {name}"
        ))),
    }
}

pub(crate) fn state_value(
    arena: &mut ValueArena,
    name: &str,
    json: json,
) -> Result<Value, ExternError> {
    let typ = typ::make::var(
        crate::phrase!(node: name.to_owned(), span: Span::default()),
        vec![],
    );
    Ok(make::external(
        arena,
        typ.node.into(),
        json,
        Span::default(),
    )?)
}

pub(crate) fn param_names(
    arena: &ValueArena,
    value_names: Value,
) -> Result<Vec<String>, ExternError> {
    get::list(arena, &value_names)?
        .iter()
        .map(|value_name| {
            get::text(arena, value_name)
                .map(str::to_owned)
                .map_err(ExternError::from)
        })
        .collect()
}

pub(crate) fn eval_func_lctk<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    values: &[Value],
) -> Result<Vec<Value>, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let [value_ctx, value_name, value_names_param] = values else {
        return Err(ExternError::Failure(
            "unexpected number of arguments to local compile-time known extern function call"
                .to_owned(),
        )
        .into());
    };
    let name_func = crate::lang::data::value::get::text(ctx.arena(), value_name)
        .map_err(|error| Interp::Error::from(ExternError::Failure(error.to_string())))?;
    let values_name_param = crate::lang::data::value::get::list(ctx.arena(), value_names_param)
        .map_err(|error| Interp::Error::from(ExternError::Failure(error.to_string())))?;
    let names_param = values_name_param
        .iter()
        .map(|value| {
            crate::lang::data::value::get::text(ctx.arena(), value)
                .map_err(|error| Interp::Error::from(ExternError::Failure(error.to_string())))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let has_message = match (name_func, names_param.as_slice()) {
        ("static_assert", ["check", "message"]) => true,
        ("static_assert", ["check"]) => false,
        _ => {
            return Err(ExternError::Failure(format!(
                "unsupported local compile-time known extern function call: {name_func}({})",
                names_param.join(", ")
            ))
            .into());
        }
    };
    let value = core::func::static_assert(ctx, value_ctx, has_message)?;
    Ok(vec![value])
}
