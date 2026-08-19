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
        values: &[ValueRef],
    ) -> Result<ValueRef, InterfaceError> {
        self.calls.set(self.calls.get() + 1);
        let value = values
            .first()
            .cloned()
            .unwrap_or_else(|| make::text(format!("builtin:{}", id.node), span("builtin-result")));
        add(value.clone());
        Ok(value)
    }

    fn checkpoint(&self) -> u64 {
        0
    }

    fn clear(&mut self) {}
}

fn forwarding_function(name: &str, target: &str) -> sl::Def {
    let parameter_exp = il::Exp::new(
        il::ExpKind::VarE(id("value")),
        il::TypKind::TextT,
        span("parameter-exp"),
    );
    let parameter = Spanned::new(
        sl::ParamKind::ExpP(make_type::text_type(), parameter_exp.clone()),
        span("parameter"),
    );
    let argument = Spanned::new(il::ArgKind::ExpA(parameter_exp), span("argument"));
    let call = il::Exp::new(
        il::ExpKind::CallE(id(target), Vec::new(), vec![argument]),
        il::TypKind::TextT,
        span("call"),
    );
    let return_instr = sl::Instr::new(sl::InstrKind::ReturnI(call), 1, span("return"));
    Spanned::new(
        sl::DefKind::FuncDecD((
            id(name),
            Vec::new(),
            vec![parameter],
            make_type::text_type(),
            vec![return_instr],
            None,
            Vec::new(),
        )),
        span("forwarding-function"),
    )
}

fn definition_parameter(name: &str) -> sl::Param {
    Spanned::new(
        sl::ParamKind::DefP(
            id(name),
            Vec::new(),
            vec![Spanned::new(
                sl::ParamKind::ExpP(
                    make_type::text_type(),
                    il::Exp::new(
                        il::ExpKind::VarE(id("argument")),
                        il::TypKind::TextT,
                        span("signature-argument"),
                    ),
                ),
                span("signature-parameter"),
            )],
            make_type::text_type(),
        ),
        span("definition-parameter"),
    )
}

fn higher_order_forwarders() -> [sl::Def; 2] {
    let value = il::Exp::new(
        il::ExpKind::VarE(id("value")),
        il::TypKind::TextT,
        span("value"),
    );
    let value_parameter = || {
        Spanned::new(
            sl::ParamKind::ExpP(make_type::text_type(), value.clone()),
            span("value-parameter"),
        )
    };
    let call = |target: &str, function: &str| {
        il::Exp::new(
            il::ExpKind::CallE(
                id(target),
                Vec::new(),
                vec![
                    Spanned::new(il::ArgKind::DefA(id(function)), span("function-argument")),
                    Spanned::new(il::ArgKind::ExpA(value.clone()), span("value-argument")),
                ],
            ),
            il::TypKind::TextT,
            span("higher-order-call"),
        )
    };
    let apply_call = il::Exp::new(
        il::ExpKind::CallE(
            id("function"),
            Vec::new(),
            vec![Spanned::new(
                il::ArgKind::ExpA(value.clone()),
                span("apply-argument"),
            )],
        ),
        il::TypKind::TextT,
        span("apply-call"),
    );
    let apply = Spanned::new(
        sl::DefKind::FuncDecD((
            id("apply"),
            Vec::new(),
            vec![definition_parameter("function"), value_parameter()],
            make_type::text_type(),
            vec![sl::Instr::new(
                sl::InstrKind::ReturnI(apply_call),
                2,
                span("apply-return"),
            )],
            None,
            Vec::new(),
        )),
        span("apply"),
    );
    let forward = Spanned::new(
        sl::DefKind::FuncDecD((
            id("forward_higher_order"),
            Vec::new(),
            vec![definition_parameter("forwarded"), value_parameter()],
            make_type::text_type(),
            vec![sl::Instr::new(
                sl::InstrKind::ReturnI(call("apply", "forwarded")),
                3,
                span("forward-return"),
            )],
            None,
            Vec::new(),
        )),
        span("forward"),
    );
    [apply, forward]
}

struct RecordingExtern {
    calls: Rc<Cell<usize>>,
}

impl Extern for RecordingExtern {
    fn eval_rel(
        &mut self,
        _spec: &mut dyn p4spec_rust::interface::SpecCall,
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
        _spec: &mut dyn p4spec_rust::interface::SpecCall,
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
fn defined_function_frame_assigns_parameters_and_runs_nested_call_expression() {
    let interface_calls = Rc::new(Cell::new(0));
    let mut interpreter = Interpreter::new(
        &[builtin("target"), forwarding_function("forward", "target")],
        Options {
            cache: false,
            deterministic: false,
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
    let first = make::text("first".to_owned(), span("first"));
    let second = make::text("second".to_owned(), span("second"));

    let result = interpreter
        .eval_func("forward", &[], std::slice::from_ref(&first))
        .unwrap();
    assert!(Rc::ptr_eq(&result, &first));
    let result = interpreter
        .eval_func("forward", &[], std::slice::from_ref(&second))
        .unwrap();
    assert!(Rc::ptr_eq(&result, &second));
    assert_eq!(interface_calls.get(), 2);
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
    assert!(error.message.contains("did not return a value"));
}

#[test]
fn definition_parameter_is_resolved_in_the_callers_local_frame() {
    let interface_calls = Rc::new(Cell::new(0));
    let [apply, forward] = higher_order_forwarders();
    let mut interpreter = Interpreter::new(
        &[builtin("target"), apply, forward],
        Options {
            cache: false,
            deterministic: false,
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
    let function = make::func(
        id("target"),
        Vec::new(),
        vec![make_type::text_type()],
        make_type::text_type(),
        span("function-value"),
    );
    let value = make::text("forwarded".to_owned(), span("value"));

    let result = interpreter
        .eval_func("forward_higher_order", &[], &[function, value.clone()])
        .unwrap();
    assert!(Rc::ptr_eq(&result, &value));
    assert_eq!(interface_calls.get(), 1);
}
