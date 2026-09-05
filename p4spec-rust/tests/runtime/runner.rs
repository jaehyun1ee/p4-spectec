use std::{cell::Cell, rc::Rc, sync::Mutex};

use p4spec_rust::{
    interface::builtin::BuiltinErrorKind,
    lang::common::source::Span,
    lang::data::value::{self, Value, get},
    lang::il::ast::Typ,
    phrase,
    runner::{
        BuiltinInterface, Extern, ExternError, Interface, InterfaceError, Interpreter, NullExtern,
        NullInterface, Runner, RunnerContext,
    },
};
use thiserror::Error;

static FRESH_BUILTIN: Mutex<()> = Mutex::new(());

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
struct FixtureState {
    next: u64,
}

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
            "next_interp" => {
                let state = context.interp_state();
                let next = state.next;
                state.next += 1;
                Ok(value::make::text(next.to_string(), Span::default()))
            }
            "next_extern" => {
                let (value, _) = context.call_extern_func("next", targs, values)?;
                Ok(value)
            }
            "next_builtin" => {
                let id = id("fresh_typeId");
                let (value, _) = context.call_builtin(&id, targs, values)?;
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

    fn clear(state: &mut Self::State) {
        state.next = 0;
    }
}

#[derive(Default)]
struct FixtureExtern {
    next: Cell<u64>,
}

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
            "next" => {
                let next = self.next.get();
                self.next.set(next + 1);
                let value = value::make::text(next.to_string(), Span::default());
                Ok((value, true))
            }
            _ => {
                let error = ExternError::Failure(name.to_owned());
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
        let error = ExternError::Failure(name.to_owned());
        Err(error.into())
    }

    fn clear(&mut self) {
        self.next.set(0);
    }
}

fn id(name: &str) -> p4spec_rust::lang::il::ast::Id {
    phrase!(node: name.to_owned(), span: Span::default())
}

#[test]
fn test_null_interface_reports_configuration_failure() {
    let error = NullInterface
        .call_builtin(&id("sum_int"), &[], &[])
        .unwrap_err();

    assert!(matches!(error, InterfaceError::NotConfigured));
}

#[test]
fn test_builtin_interface_reports_side_effects_and_clears() {
    let _guard = FRESH_BUILTIN.lock().unwrap();
    let mut interface = BuiltinInterface::new();
    interface.clear();
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
fn test_builtin_interface_preserves_builtin_failures() {
    let error = BuiltinInterface::new()
        .call_builtin(&id("sum_int"), &[], &[])
        .unwrap_err();

    assert!(matches!(
        error,
        InterfaceError::Builtin(error)
            if matches!(error.kind, BuiltinErrorKind::ArityMismatch { .. })
    ));
}

#[test]
fn test_runner_statically_composes_its_components() {
    let mut runner = Runner::<FixtureInterpreter, NullInterface, NullExtern>::new(
        (),
        FixtureState::default(),
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
        FixtureState::default(),
        NullInterface,
        FixtureExtern::default(),
    );

    let value = runner.eval_func("outer", &[], &[]).unwrap();

    assert_eq!(get::text(&value), Ok("done"));
}

#[test]
fn test_extern_reports_side_effects_with_each_result() {
    let mut runner = Runner::<FixtureInterpreter, NullInterface, FixtureExtern>::new(
        (),
        FixtureState::default(),
        NullInterface,
        FixtureExtern::default(),
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
        FixtureState::default(),
        NullInterface,
        NullExtern,
    );

    let error = runner.eval_func("extern", &[], &[]).unwrap_err();

    assert!(matches!(
        error,
        FixtureError::Extern(ExternError::NotConfigured)
    ));
}

#[test]
fn test_runner_clear_resets_every_component() {
    let _guard = FRESH_BUILTIN.lock().unwrap();
    let mut runner = Runner::<FixtureInterpreter, BuiltinInterface, FixtureExtern>::new(
        (),
        FixtureState::default(),
        BuiltinInterface::new(),
        FixtureExtern::default(),
    );
    runner.clear();

    assert_eq!(eval_text(&mut runner, "next_interp"), "0");
    assert_eq!(eval_text(&mut runner, "next_interp"), "1");
    assert_eq!(eval_text(&mut runner, "next_extern"), "0");
    assert_eq!(eval_text(&mut runner, "next_extern"), "1");
    assert_eq!(eval_text(&mut runner, "next_builtin"), "FRESH__0");
    assert_eq!(eval_text(&mut runner, "next_builtin"), "FRESH__1");

    runner.clear();

    assert_eq!(eval_text(&mut runner, "next_interp"), "0");
    assert_eq!(eval_text(&mut runner, "next_extern"), "0");
    assert_eq!(eval_text(&mut runner, "next_builtin"), "FRESH__0");
}

fn eval_text(
    runner: &mut Runner<FixtureInterpreter, BuiltinInterface, FixtureExtern>,
    name: &str,
) -> String {
    let value = runner.eval_func(name, &[], &[]).unwrap();
    get::text(&value).unwrap().to_owned()
}
