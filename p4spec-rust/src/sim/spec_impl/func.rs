use std::rc::Rc;

use crate::{
    lang::{
        common::source::Span,
        data::value::{Value, make},
    },
    runner::{Extern, Interface, Interpreter, RunnerContext},
};

pub fn find_var_value_t<S, I, E>(
    context: &mut RunnerContext<'_, S, I, E>,
    value_cursor: &Rc<Value>,
    value_ctx: &Rc<Value>,
    name: &str,
) -> Result<Rc<Value>, S::Error>
where
    I: Interface,
    E: Extern,
    S: Interpreter<I, E>,
{
    let value_name = make::text(name.to_owned(), Span::default());
    let value_name = make::case__(
        "_BARE nameIR",
        vec![value_name],
        "prefixedNameIR",
        Span::default(),
    );
    context.call_func(
        "find_var_value_t",
        &[],
        &[value_name, Rc::clone(value_cursor), Rc::clone(value_ctx)],
    )
}

pub fn find_var_value_t_local<S, I, E>(
    context: &mut RunnerContext<'_, S, I, E>,
    value_ctx: &Rc<Value>,
    name: &str,
) -> Result<Rc<Value>, S::Error>
where
    I: Interface,
    E: Extern,
    S: Interpreter<I, E>,
{
    let value_cursor = make::case__("LOCAL", Vec::new(), "cursor", Span::default());
    find_var_value_t(context, &value_cursor, value_ctx, name)
}
