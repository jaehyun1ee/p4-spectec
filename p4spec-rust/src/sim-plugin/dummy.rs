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
                let id = crate::phrase!(
                    node: name_typ.to_owned(),
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
