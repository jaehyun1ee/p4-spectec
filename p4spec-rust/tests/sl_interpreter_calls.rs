use std::{cell::Cell, rc::Rc};

use p4spec_rust::{
    domain::source::{Region, Spanned},
    interface::{Extern, ExternError, Interface, InterfaceError},
    interp::sl::{Interpreter, Options},
    lang::{il::ast as il, sl::ast as sl},
    runtime::{
        r#type::typ::make as make_type,
        value::{ValueRef, get, make},
    },
};

fn span(file: &str) -> Region {
    Region::for_file(file)
}

fn id(name: &str) -> il::Id {
    Spanned::new(name.to_owned(), span(name))
}

fn builtin(name: &str) -> sl::Def {
    Spanned::new(
        sl::DefKind::BuiltinDecD((
            id(name),
            Vec::new(),
            Vec::new(),
            make_type::text_type(),
            Vec::new(),
        )),
        span("builtin"),
    )
}

fn external(name: &str) -> sl::Def {
    Spanned::new(
        sl::DefKind::ExternDecD((
            id(name),
            Vec::new(),
            Vec::new(),
            make_type::text_type(),
            Vec::new(),
        )),
        span("extern"),
    )
}

fn defined(name: &str) -> sl::Def {
    Spanned::new(
        sl::DefKind::FuncDecD((
            id(name),
            Vec::new(),
            Vec::new(),
            make_type::text_type(),
            Vec::new(),
            None,
            Vec::new(),
        )),
        span("defined"),
    )
}

struct RecordingInterface {
    calls: Rc<Cell<usize>>,
}

impl Interface for RecordingInterface {
    fn call_builtin(
        &mut self,
        add: &mut dyn FnMut(ValueRef),
        id: &il::Id,
        _type_args: &[il::Typ],
        _values: &[ValueRef],
    ) -> Result<ValueRef, InterfaceError> {
        self.calls.set(self.calls.get() + 1);
        let value = make::text(format!("builtin:{}", id.node), span("builtin-result"));
        add(value.clone());
        Ok(value)
    }

    fn checkpoint(&self) -> u64 {
        0
    }

    fn clear(&mut self) {}
}

struct RecordingExtern {
    calls: Rc<Cell<usize>>,
}

impl Extern for RecordingExtern {
    fn eval_rel(
        &mut self,
        _name: &str,
        _values: &[ValueRef],
    ) -> Result<Vec<ValueRef>, ExternError> {
        Err(ExternError::new(
            span("relation"),
            "unexpected relation call",
        ))
    }

    fn eval_func(
        &mut self,
        name: &str,
        _type_args: &[il::Typ],
        _values: &[ValueRef],
    ) -> Result<ValueRef, ExternError> {
        self.calls.set(self.calls.get() + 1);
        Ok(make::text(format!("extern:{name}"), span("extern-result")))
    }

    fn checkpoint(&self) -> u64 {
        0
    }

    fn clear(&mut self) {}
}

#[test]
fn public_eval_func_dispatches_builtin_and_extern_definitions() {
    let interface_calls = Rc::new(Cell::new(0));
    let extern_calls = Rc::new(Cell::new(0));
    let mut interpreter = Interpreter::new(
        &[builtin("local_builtin"), external("local_extern")],
        Options {
            cache: true,
            deterministic: false,
            guard: false,
        },
        RecordingInterface {
            calls: interface_calls.clone(),
        },
        RecordingExtern {
            calls: extern_calls.clone(),
        },
    )
    .unwrap();

    let builtin_result = interpreter.eval_func("local_builtin", &[], &[]).unwrap();
    assert_eq!(get::text(&builtin_result), Ok("builtin:local_builtin"));
    let extern_result = interpreter.eval_func("local_extern", &[], &[]).unwrap();
    assert_eq!(get::text(&extern_result), Ok("extern:local_extern"));
    assert_eq!(interface_calls.get(), 1);
    assert_eq!(extern_calls.get(), 1);
}

#[test]
fn each_public_call_clears_the_call_cache() {
    let interface_calls = Rc::new(Cell::new(0));
    let mut interpreter = Interpreter::new(
        &[builtin("cached")],
        Options {
            cache: true,
            deterministic: true,
            guard: false,
        },
        RecordingInterface {
            calls: interface_calls.clone(),
        },
        RecordingExtern {
            calls: Rc::new(Cell::new(0)),
        },
    )
    .unwrap();

    interpreter.eval_func("cached", &[], &[]).unwrap();
    interpreter.eval_func("cached", &[], &[]).unwrap();
    assert_eq!(interface_calls.get(), 2);
    assert!(interpreter.options().deterministic);
}

#[test]
fn missing_and_not_yet_executable_function_kinds_return_typed_errors() {
    let mut interpreter = Interpreter::new(
        &[defined("pending")],
        Options::default(),
        RecordingInterface {
            calls: Rc::new(Cell::new(0)),
        },
        RecordingExtern {
            calls: Rc::new(Cell::new(0)),
        },
    )
    .unwrap();
    let error = interpreter.eval_func("missing", &[], &[]).unwrap_err();
    assert!(error.message.contains("function `missing` is undefined"));
    let error = interpreter.eval_func("pending", &[], &[]).unwrap_err();
    assert!(error.message.contains("execution is not implemented"));
}
