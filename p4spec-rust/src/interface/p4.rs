use crate::{
    lang::il::ast::{Id, Typ},
    runtime::value::{Value, ValueRef, make},
};

use super::{
    Interface, InterfaceError,
    builtin::{call::Builtins, extract, return_value},
};

// P4

pub struct P4Interface {
    unparser: Box<dyn Fn(&Value) -> String>,
    builtins: Builtins,
}

impl P4Interface {
    pub fn new(unparser: impl Fn(&Value) -> String + 'static) -> Self {
        Self {
            unparser: Box::new(unparser),
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
        let text = (self.unparser)(value);
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
