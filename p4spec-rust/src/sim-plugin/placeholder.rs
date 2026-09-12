use crate::{
    lang::{
        common::source::Span,
        data::{
            typ::make as make_typ,
            value::{Value, make as make_value},
        },
        il::ast::Typ,
    },
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
};

use super::core;

pub struct Placeholder;

impl Extern for Placeholder {
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
        Ok((vec![value], false))
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
            "init_objectState" => {
                let id = crate::phrase!(
                    node: "objectState".to_owned(),
                    span: Span::default(),
                );
                let typ = make_typ::var(id, Vec::new());
                let value = make_value::external(
                    ctx.arena_mut(),
                    typ.node.into(),
                    crate::util::json::json::Null,
                    Span::default(),
                )
                .map_err(ExternError::from)
                .map_err(Interp::Error::from)?;
                Ok((value, false))
            }
            _ => Err(ExternError::Failure(format!("unimplemented extern function: {name}")).into()),
        }
    }

    fn clear(&mut self) {}
}
