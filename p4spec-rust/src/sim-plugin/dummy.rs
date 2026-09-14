use crate::lang::data::value::external::encode;
use crate::{
    lang::{data::value::Value, il::ast::Typ},
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
};

pub struct Dummy;

impl Extern for Dummy {
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
        if name != "ExternFunctionCall_eval_lctk" {
            return Err(
                ExternError::Failure(format!("unimplemented extern relation: {name}")).into(),
            );
        }
        Ok((super::externs::eval_func_lctk(ctx, values)?, false))
    }

    fn eval_func<Interp, Iface>(
        &self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        name: &str,
        _targs: &[Typ],
        _values: &[Value],
    ) -> Result<(Value, bool), Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        match name {
            "init_objectState" | "init_archState" => {
                let name_typ = if name == "init_archState" {
                    "archState"
                } else {
                    "objectState"
                };
                let payload = encode(ctx.arena(), &())
                    .map_err(|error| ExternError::Failure(error.to_string()))?;
                let value = super::externs::state_value(ctx.arena_mut(), name_typ, payload.into())?;
                Ok((value, false))
            }
            _ => Err(ExternError::Failure(format!("unimplemented extern function: {name}")).into()),
        }
    }

    fn clear(&mut self) {}
}
