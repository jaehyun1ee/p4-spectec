use std::rc::Rc;

use p4spec_rust::{
    lang::common::source::{Position, Span},
    lang::data::value::{self, Value, get},
    lang::il::ast::Typ,
    phrase,
    runner::{
        BuiltinInterface, Extern, ExternError, Interface, InterfaceError, InterfaceErrorKind,
        Interpreter, NullExtern, NullInterface, Runner, RunnerContext,
    },
};
use thiserror::Error;

#[derive(Debug, Error)]
enum FixtureError {
    #[error(transparent)]
    Interface(#[from] InterfaceError),
    #[error(transparent)]
    Extern(#[from] ExternError),
    #[error("unknown fixture call: {0}")]
    Unknown(String),
}

#[derive(Default)]
struct FixtureState;

struct FixtureInterpreter;

impl<I, E> Interpreter<I, E> for FixtureInterpreter
where
    I: Interface,
    E: Extern,
{
    type Spec = ();
    type State = FixtureState;
    type Error = FixtureError;

    fn eval_func(
        context: &mut RunnerContext<'_, Self, I, E>,
        name: &str,
        targs: &[Typ],
        values: &[Rc<Value>],
    ) -> Result<Rc<Value>, Self::Error> {
        match name {
            "done" => Ok(value::make::text("done".to_owned(), Span::default())),
            "outer" => {
                let (value, _) = context.call_extern_func("first", targs, values)?;
                Ok(value)
            }
            "inner" => {
                let (value, _) = context.call_extern_func("second", targs, values)?;
                Ok(value)
            }
            "pure_effect" => {
                let (_, side_effected) = context.call_extern_func("pure", targs, values)?;
                Ok(value::make::bool(side_effected, Span::default()))
            }
            "impure_effect" => {
                let (_, side_effected) = context.call_extern_func("impure", targs, values)?;
                Ok(value::make::bool(side_effected, Span::default()))
            }
            "extern" => {
                let (value, _) = context.call_extern_func("missing", targs, values)?;
                Ok(value)
            }
            _ => Err(FixtureError::Unknown(name.to_owned())),
        }
    }

    fn eval_rel(
        _context: &mut RunnerContext<'_, Self, I, E>,
        name: &str,
        _values: &[Rc<Value>],
    ) -> Result<Vec<Rc<Value>>, Self::Error> {
        Err(FixtureError::Unknown(name.to_owned()))
    }

    fn clear(_state: &mut Self::State) {}
}

struct FixtureExtern;

impl Extern for FixtureExtern {
    fn eval_func<S, I>(
        &self,
        context: &mut RunnerContext<'_, S, I, Self>,
        name: &str,
        targs: &[Typ],
        values: &[Rc<Value>],
    ) -> Result<(Rc<Value>, bool), S::Error>
    where
        I: Interface,
        S: Interpreter<I, Self>,
    {
        match name {
            "first" => {
                let value = context.call_func("inner", targs, values)?;
                Ok((value, false))
            }
            "second" => {
                let value = context.call_func("done", targs, values)?;
                Ok((value, false))
            }
            "pure" => {
                let value = value::make::bool(false, Span::default());
                Ok((value, false))
            }
            "impure" => {
                let value = value::make::bool(true, Span::default());
                Ok((value, true))
            }
            _ => {
                let error = ExternError {
                    kind: p4spec_rust::runner::ExternErrorKind::Failure(name.to_owned()),
                    span: Span::default(),
                };
                Err(error.into())
            }
        }
    }

    fn eval_rel<S, I>(
        &self,
        _context: &mut RunnerContext<'_, S, I, Self>,
        name: &str,
        _values: &[Rc<Value>],
    ) -> Result<(Vec<Rc<Value>>, bool), S::Error>
    where
        I: Interface,
        S: Interpreter<I, Self>,
    {
        let error = ExternError {
            kind: p4spec_rust::runner::ExternErrorKind::Failure(name.to_owned()),
            span: Span::default(),
        };
        Err(error.into())
    }

    fn clear(&mut self) {}
}

fn id(name: &str) -> p4spec_rust::lang::il::ast::Id {
    phrase!(node: name.to_owned(), span: Span::default())
}

#[test]
fn test_null_interface_reports_configuration_failure() {
    let error = NullInterface
        .call_builtin(&id("sum_int"), &[], &[])
        .unwrap_err();

    assert!(matches!(error.kind, InterfaceErrorKind::NotConfigured));
}

#[test]
fn test_builtin_interface_reports_side_effects_and_clears() {
    let mut interface = BuiltinInterface::new();
    let (value, side_effected) = interface
        .call_builtin(&id("fresh_typeId"), &[], &[])
        .unwrap();

    assert_eq!(get::text(&value), Ok("FRESH__0"));
    assert!(side_effected);

    interface.clear();
    let (value, side_effected) = interface
        .call_builtin(&id("fresh_typeId"), &[], &[])
        .unwrap();
    assert_eq!(get::text(&value), Ok("FRESH__0"));
    assert!(side_effected);
}

#[test]
fn test_builtin_interface_locates_builtin_failures_at_the_call() {
    let span = Span::new(
        Position::new("test.spec", 3, 4),
        Position::new("test.spec", 3, 11),
    );
    let id = phrase!(node: "sum_int".to_owned(), span: span.clone());
    let error = BuiltinInterface::new()
        .call_builtin(&id, &[], &[])
        .unwrap_err();

    assert_eq!(error.span, span);
    assert!(matches!(error.kind, InterfaceErrorKind::Builtin(_)));
}

#[test]
fn test_runner_statically_composes_its_components() {
    let mut runner = Runner::<FixtureInterpreter, NullInterface, NullExtern>::new(
        (),
        FixtureState,
        NullInterface,
        NullExtern,
    );

    let value = runner.eval_func("done", &[], &[]).unwrap();

    assert_eq!(get::text(&value), Ok("done"));
}

#[test]
fn test_extern_can_reenter_the_interpreter() {
    let mut runner = Runner::<FixtureInterpreter, NullInterface, FixtureExtern>::new(
        (),
        FixtureState,
        NullInterface,
        FixtureExtern,
    );

    let value = runner.eval_func("outer", &[], &[]).unwrap();

    assert_eq!(get::text(&value), Ok("done"));
}

#[test]
fn test_extern_reports_side_effects_with_each_result() {
    let mut runner = Runner::<FixtureInterpreter, NullInterface, FixtureExtern>::new(
        (),
        FixtureState,
        NullInterface,
        FixtureExtern,
    );

    let value_pure = runner.eval_func("pure_effect", &[], &[]).unwrap();
    let value_impure = runner.eval_func("impure_effect", &[], &[]).unwrap();

    assert_eq!(get::bool(&value_pure), Ok(false));
    assert_eq!(get::bool(&value_impure), Ok(true));
}

#[test]
fn test_null_extern_reports_configuration_failure() {
    let mut runner = Runner::<FixtureInterpreter, NullInterface, NullExtern>::new(
        (),
        FixtureState,
        NullInterface,
        NullExtern,
    );

    let error = runner.eval_func("extern", &[], &[]).unwrap_err();

    assert!(matches!(
        error,
        FixtureError::Extern(ExternError {
            kind: p4spec_rust::runner::ExternErrorKind::NotConfigured,
            ..
        })
    ));
}
