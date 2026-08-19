use crate::{
    domain::{
        atom::Atom,
        external_data::ExternalData,
        mixfix::{Mixfix, Mixop},
        source::{Region, Spanned},
    },
    lang::il::ast::Typ,
    runtime::{
        r#type::typ::make as make_type,
        value::{Value, ValueRef, get, make},
    },
};

use super::{Extern, ExternError, SpecCall};

pub struct PlaceholderExtern;

impl PlaceholderExtern {
    pub fn new() -> Self {
        Self
    }

    fn bool_mixop() -> Mixop {
        Mixfix::Seq(vec![
            Mixfix::Atom(Spanned::new(Atom::Tag("B".to_owned()), Region::none())),
            Mixfix::Arg(()),
        ])
    }

    fn string_mixop() -> Mixop {
        let quote = Atom::Operator("\"".to_owned());
        Mixfix::Seq(vec![
            Mixfix::Atom(Spanned::new(quote.clone(), Region::none())),
            Mixfix::Arg(()),
            Mixfix::Atom(Spanned::new(quote, Region::none())),
        ])
    }

    fn unpack_p4_bool(value: &Value) -> Result<bool, ExternError> {
        let value_case = get::case(value)
            .map_err(|error| ExternError::new(Region::none(), error.to_string()))?;
        if value_case.split().0 != Self::bool_mixop() {
            return Err(ExternError::new(Region::none(), "expected a P4 bool"));
        }
        let args = value_case.args();
        let [value] = args.as_slice() else {
            return Err(ExternError::new(Region::none(), "expected a P4 bool"));
        };
        get::bool(value).map_err(|error| ExternError::new(Region::none(), error.to_string()))
    }

    fn unpack_p4_string(value: &Value) -> Result<String, ExternError> {
        let value_case = get::case(value)
            .map_err(|error| ExternError::new(Region::none(), error.to_string()))?;
        if value_case.split().0 != Self::string_mixop() {
            return Err(ExternError::new(Region::none(), "expected a P4 string"));
        }
        let args = value_case.args();
        let [value] = args.as_slice() else {
            return Err(ExternError::new(Region::none(), "expected a P4 string"));
        };
        get::text(value)
            .map(str::to_owned)
            .map_err(|error| ExternError::new(Region::none(), error.to_string()))
    }

    fn eval_extern_func_lctk_call(
        &mut self,
        spec: &mut dyn SpecCall,
        values: &[ValueRef],
    ) -> Result<Vec<ValueRef>, ExternError> {
        // Static assert evaluates a boolean expression at compilation time. If
        // the expression evaluates to false, compilation is stopped and the
        // corresponding message is printed. Like P4, the message is optional.
        let [value_ctx, value_name_func, value_names_param] = values else {
            return Err(ExternError::new(
                Region::none(),
                "unexpected number of arguments to local compile-time known extern function call",
            ));
        };
        let name_func = get::text(value_name_func)
            .map_err(|error| ExternError::new(Region::none(), error.to_string()))?;
        let names = get::list(value_names_param)
            .map_err(|error| ExternError::new(Region::none(), error.to_string()))?;
        let names_param = names
            .iter()
            .map(|value| {
                get::text(value)
                    .map(str::to_owned)
                    .map_err(|error| ExternError::new(Region::none(), error.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let has_message = match (name_func, names_param.as_slice()) {
            ("static_assert", [check, message]) if check == "check" && message == "message" => true,
            ("static_assert", [check]) if check == "check" => false,
            _ => {
                return Err(ExternError::new(
                    Region::none(),
                    format!(
                        "unsupported local compile-time known extern function call: {}({})",
                        name_func,
                        names_param.join(", ")
                    ),
                ));
            }
        };
        let value_check = Self::find_var_value_t_local(spec, value_ctx, "check")?;
        if Self::unpack_p4_bool(&value_check)? {
            return Ok(vec![value_check]);
        }
        let message = if has_message {
            let value_message = Self::find_var_value_t_local(spec, value_ctx, "message")?;
            Self::unpack_p4_string(&value_message)?
        } else {
            "static_assert failed".to_owned()
        };
        Err(ExternError::new(Region::none(), message))
    }

    fn find_var_value_t_local(
        spec: &mut dyn SpecCall,
        value_ctx: &ValueRef,
        name: &str,
    ) -> Result<ValueRef, ExternError> {
        let span = Region::none();
        let prefixed_name = make::case(
            &crate::runtime::r#type::typ::make::var_type(
                Spanned::new("prefixedNameIR".to_owned(), span.clone()),
                Vec::new(),
            ),
            Mixfix::Seq(vec![
                Mixfix::Atom(Spanned::new(Atom::Tag("BARE".to_owned()), span.clone())),
                Mixfix::Arg(make::text(name.to_owned(), span.clone())),
            ]),
            span.clone(),
        );
        let cursor = make::case(
            &crate::runtime::r#type::typ::make::var_type(
                Spanned::new("cursor".to_owned(), span.clone()),
                Vec::new(),
            ),
            Mixfix::Atom(Spanned::new(
                Atom::Keyword("LOCAL".to_owned()),
                span.clone(),
            )),
            span,
        );
        spec.eval_func(
            "find_var_value_t",
            &[],
            &[prefixed_name, cursor, value_ctx.clone()],
        )
    }

    fn external_state(name: &str) -> ValueRef {
        make::external(
            &make_type::var_type(Spanned::new(name.to_owned(), Region::none()), Vec::new()),
            ExternalData::Null,
            Region::none(),
        )
    }
}

impl Default for PlaceholderExtern {
    fn default() -> Self {
        Self::new()
    }
}

impl Extern for PlaceholderExtern {
    fn eval_rel(
        &mut self,
        spec: &mut dyn SpecCall,
        name: &str,
        values: &[ValueRef],
    ) -> Result<Vec<ValueRef>, ExternError> {
        match name {
            "ExternFunctionCall_eval_lctk" => self.eval_extern_func_lctk_call(spec, values),
            _ => Err(ExternError::new(
                Region::none(),
                format!("unimplemented extern relation: {name}"),
            )),
        }
    }

    fn eval_func(
        &mut self,
        _spec: &mut dyn SpecCall,
        name: &str,
        _type_args: &[Typ],
        _values: &[ValueRef],
    ) -> Result<ValueRef, ExternError> {
        match name {
            "init_objectState" => Ok(Self::external_state("objectState")),
            "init_archState" => Ok(Self::external_state("archState")),
            _ => Err(ExternError::new(
                Region::none(),
                format!("unimplemented extern function: {name}"),
            )),
        }
    }

    fn checkpoint(&self) -> u64 {
        0
    }

    fn clear(&mut self) {}
}
