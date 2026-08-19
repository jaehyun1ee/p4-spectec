use std::rc::Rc;

use p4spec_rust::{
    domain::{
        mixfix::Mixfix,
        source::{Region, Spanned},
    },
    interface::{Extern, ExternError, NullInterface},
    interp::sl::{Interpreter, Options},
    lang::{il::ast as il, sl::ast as sl},
    runtime::value::{ValueRef, make},
};

fn span(file: &str) -> Region {
    Region::for_file(file)
}

fn id(name: &str) -> il::Id {
    Spanned::new(name.to_owned(), span(name))
}

fn signature() -> sl::RelSignature {
    (
        Spanned::new(Mixfix::Seq(Vec::new()), span("signature")),
        Vec::new(),
    )
}

fn variable(name: &str) -> il::Exp {
    il::Exp::new(il::ExpKind::VarE(id(name)), il::TypKind::TextT, span(name))
}

fn defined_identity() -> sl::Def {
    let value = variable("value");
    Spanned::new(
        sl::DefKind::RelD((
            id("identity"),
            signature(),
            vec![value.clone()],
            vec![sl::Instr::new(
                sl::InstrKind::ResultI(signature(), vec![value]),
                1,
                span("result"),
            )],
            None,
            Vec::new(),
        )),
        span("identity"),
    )
}

fn extern_relation() -> sl::Def {
    Spanned::new(
        sl::DefKind::ExternRelD((id("external"), signature(), Vec::new(), Vec::new())),
        span("external"),
    )
}

struct EchoExtern;

impl Extern for EchoExtern {
    fn eval_rel(
        &mut self,
        _spec: &mut dyn p4spec_rust::interface::SpecCall,
        _name: &str,
        values: &[ValueRef],
    ) -> Result<Vec<ValueRef>, ExternError> {
        Ok(values.to_vec())
    }

    fn eval_func(
        &mut self,
        _spec: &mut dyn p4spec_rust::interface::SpecCall,
        _name: &str,
        _type_args: &[il::Typ],
        _values: &[ValueRef],
    ) -> Result<ValueRef, ExternError> {
        Err(ExternError::new(
            span("function"),
            "unexpected function call",
        ))
    }

    fn checkpoint(&self) -> u64 {
        0
    }

    fn clear(&mut self) {}
}

#[test]
fn public_eval_rel_dispatches_defined_and_extern_relations() {
    let mut interpreter = Interpreter::new(
        &[defined_identity(), extern_relation()],
        Options {
            cache: true,
            deterministic: false,
            guard: false,
        },
        NullInterface,
        EchoExtern,
    )
    .unwrap();
    let value = make::text("value".to_owned(), span("value"));

    let defined = interpreter
        .eval_rel("identity", std::slice::from_ref(&value))
        .unwrap();
    assert_eq!(defined.len(), 1);
    assert!(Rc::ptr_eq(&defined[0], &value));
    let external = interpreter
        .eval_rel("external", std::slice::from_ref(&value))
        .unwrap();
    assert_eq!(external.len(), 1);
    assert!(Rc::ptr_eq(&external[0], &value));
}

#[test]
fn relation_arity_and_missing_result_are_typed_failures() {
    let no_result = Spanned::new(
        sl::DefKind::RelD((
            id("no_result"),
            signature(),
            Vec::new(),
            Vec::new(),
            None,
            Vec::new(),
        )),
        span("no-result"),
    );
    let mut interpreter = Interpreter::new(
        &[defined_identity(), no_result],
        Options::default(),
        NullInterface,
        EchoExtern,
    )
    .unwrap();

    let error = interpreter.eval_rel("identity", &[]).unwrap_err();
    assert!(error.is_unmatch());
    let error = interpreter.eval_rel("no_result", &[]).unwrap_err();
    assert!(error.is_unmatch());
}

#[test]
fn eval_program_invokes_a_relation_with_the_program_as_its_only_input() {
    let mut interpreter = Interpreter::new(
        &[defined_identity()],
        Options {
            cache: false,
            deterministic: false,
            guard: false,
        },
        NullInterface,
        EchoExtern,
    )
    .unwrap();
    let program = make::text("program".to_owned(), span("program"));

    let result = interpreter.eval_program("identity", &program).unwrap();
    assert_eq!(result.len(), 1);
    assert!(Rc::ptr_eq(&result[0], &program));
}
