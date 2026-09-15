//! Interface presets assemble specification builtins
//! For example, `p4(&spec)` adds `print_` using the specification's print hints

use crate::{
    lang::{al::ast::Spec, common::source::Span, data::value},
    runner::BuiltinInterface,
};

use self::{
    builtin::{BuiltinError, BuiltinErrorKind, call::Builtins, extract},
    p4::unparse::P4Unparser,
};

pub mod builtin;
pub mod p4;

// == P4

pub fn p4(spec: &Spec) -> BuiltinInterface {
    let unparser = P4Unparser::from_al_spec(spec);
    let builtins = Builtins::with_extensions([(
        "print_",
        Box::new(move |arena, targs, values| {
            let _typ = extract::one(targs)?;
            let value = extract::one(values)?;
            let text = unparser
                .render(arena, value)
                .map_err(|error| BuiltinError {
                    kind: BuiltinErrorKind::P4Unparse(error),
                })?;
            Ok(value::make::text(arena, text, Span::default())?)
        }),
    )]);
    BuiltinInterface::new(builtins)
}
