use super::core;
use crate::{
    lang::{data::value::Value, il::ast::Typ},
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
};

// == Architecture extern operations

pub(crate) trait Impl: Extern {
    fn eval_extern_init<Interp, Iface>(
        &self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        values: &[Value],
    ) -> Result<Value, Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>;

    fn eval_extern_func_lctk_call<Interp, Iface>(
        &self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        values: &[Value],
    ) -> Result<Vec<Value>, Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        eval_func_lctk(ctx, values)
    }

    fn eval_extern_func_call<Interp, Iface>(
        &self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        values: &[Value],
    ) -> Result<Vec<Value>, Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>;

    fn eval_extern_method_call<Interp, Iface>(
        &self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        values: &[Value],
    ) -> Result<Vec<Value>, Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>;

    fn init_arch_state<Interp, Iface>(
        &self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
    ) -> Result<Value, Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>;
}

// == Runner dispatch

impl<Exn: Impl> Extern for Exn {
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
        let values = match name {
            "ExternFunctionCall_eval_lctk" => self.eval_extern_func_lctk_call(ctx, values)?,
            "ExternFunctionCall_eval" => self.eval_extern_func_call(ctx, values)?,
            "ExternMethodCall_eval" => self.eval_extern_method_call(ctx, values)?,
            _ => {
                return Err(
                    ExternError::Failure(format!("unimplemented extern relation: {name}")).into()
                );
            }
        };
        Ok((values, false))
    }

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
        let value = match name {
            "init_objectState" => self.eval_extern_init(ctx, values)?,
            "init_archState" => self.init_arch_state(ctx)?,
            _ => {
                return Err(
                    ExternError::Failure(format!("unimplemented extern function: {name}")).into()
                );
            }
        };
        Ok((value, false))
    }

    fn clear(&mut self) {}
}

// == Compile-time extern calls

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
