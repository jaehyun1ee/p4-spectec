use crate::{
    lang::il::ast::{Id, Typ},
    lang::sl::ast::Def,
    runtime::value::{Value, ValueRef, make},
};

use super::{
    Interface, InterfaceError,
    builtin::{call::Builtins, extract, return_value},
};

// P4

type Unparser = dyn Fn(&Value) -> Result<String, String>;

pub struct P4Interface {
    unparser: Box<Unparser>,
    builtins: Builtins,
}

impl P4Interface {
    pub fn new(unparser: impl Fn(&Value) -> String + 'static) -> Self {
        Self {
            unparser: Box::new(move |value| Ok(unparser(value))),
            builtins: Builtins::new(),
        }
    }

    pub fn from_sl_spec(spec: &[Def]) -> Self {
        let unparser = super::P4Unparser::from_sl_spec(spec);
        Self {
            unparser: Box::new(move |value| {
                unparser.render(value).map_err(|error| error.to_string())
            }),
            builtins: Builtins::new(),
        }
    }

    // dec $print_<X>(X) : text

    fn print(
        &self,
        add: &mut dyn FnMut(ValueRef),
        id: &Id,
        type_args: &[Typ],
        values: &[ValueRef],
    ) -> Result<ValueRef, InterfaceError> {
        let _typ = extract::one(&id.span, type_args)?;
        let value = extract::one(&id.span, values)?;
        let text = (self.unparser)(value)
            .map_err(|message| InterfaceError::new(id.span.clone(), message))?;
        return_value(add, make::text(text, crate::domain::source::Region::none()))
            .map_err(Into::into)
    }
}

impl Interface for P4Interface {
    fn call_builtin(
        &mut self,
        add: &mut dyn FnMut(ValueRef),
        id: &Id,
        type_args: &[Typ],
        values: &[ValueRef],
    ) -> Result<ValueRef, InterfaceError> {
        if id.node == "print_" {
            self.print(add, id, type_args, values)
        } else {
            self.builtins
                .invoke(add, id, type_args, values)
                .map_err(Into::into)
        }
    }

    fn checkpoint(&self) -> u64 {
        self.builtins.checkpoint()
    }

    fn clear(&mut self) {
        self.builtins.init();
    }
}
