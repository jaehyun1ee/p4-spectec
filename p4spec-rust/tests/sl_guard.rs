use p4spec_rust::{
    domain::{
        mixfix::Mixfix,
        source::{Region, Spanned},
    },
    interface::{Extern, ExternError, NullExtern, NullInterface},
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

fn variable(name: &str) -> il::Exp {
    il::Exp::new(il::ExpKind::VarE(id(name)), il::TypKind::TextT, span(name))
}

fn signature() -> sl::RelSignature {
    (
        Spanned::new(
            Mixfix::Seq(vec![
                Mixfix::Arg(make_type::text_type()),
                Mixfix::Arg(make_type::text_type()),
            ]),
            span("signature"),
        ),
        vec![0],
    )
}

fn guarded_spec() -> [sl::Def; 2] {
    let value = variable("value");
    let function = Spanned::new(
        sl::DefKind::FuncDecD((
            id("identity_text"),
            Vec::new(),
            vec![Spanned::new(
                sl::ParamKind::ExpP(make_type::text_type(), value.clone()),
                span("parameter"),
            )],
            make_type::text_type(),
            vec![sl::Instr::new(
                sl::InstrKind::ReturnI(value.clone()),
                1,
                span("return"),
            )],
            None,
            Vec::new(),
        )),
        span("function"),
    );
    let relation = Spanned::new(
        sl::DefKind::RelD((
            id("identity_text_rel"),
            signature(),
            vec![value.clone()],
            vec![sl::Instr::new(
                sl::InstrKind::ResultI(signature(), vec![value]),
                2,
                span("result"),
            )],
            None,
            Vec::new(),
        )),
        span("relation"),
    );
    [function, relation]
}

fn interpreter(guard: bool) -> Interpreter<NullInterface, NullExtern> {
    Interpreter::new(
        &guarded_spec(),
        Options {
            cache: false,
            deterministic: false,
            guard,
        },
        NullInterface,
        NullExtern,
    )
    .unwrap()
}

#[test]
fn guard_rejects_public_function_and_relation_inputs_of_the_wrong_type() {
    let value = make::bool(true, span("bool"));
    let mut guarded = interpreter(true);

    assert!(
        guarded
            .eval_func("identity_text", &[], std::slice::from_ref(&value))
            .is_err()
    );
    assert!(
        guarded
            .eval_rel("identity_text_rel", std::slice::from_ref(&value))
            .is_err()
    );
}

#[test]
fn disabling_guard_preserves_the_unchecked_runtime_path() {
    let value = make::bool(true, span("bool"));
    let mut unchecked = interpreter(false);

    let function = unchecked
        .eval_func("identity_text", &[], std::slice::from_ref(&value))
        .unwrap();
    assert_eq!(get::bool(&function), Ok(true));
    let relation = unchecked
        .eval_rel("identity_text_rel", std::slice::from_ref(&value))
        .unwrap();
    assert_eq!(get::bool(&relation[0]), Ok(true));
}

struct WrongTypeExtern;

impl Extern for WrongTypeExtern {
    fn eval_rel(
        &mut self,
        _spec: &mut dyn p4spec_rust::interface::SpecCall,
        _name: &str,
        _values: &[ValueRef],
    ) -> Result<Vec<ValueRef>, ExternError> {
        Ok(vec![make::bool(true, span("extern-rel"))])
    }

    fn eval_func(
        &mut self,
        _spec: &mut dyn p4spec_rust::interface::SpecCall,
        _name: &str,
        _type_args: &[il::Typ],
        _values: &[ValueRef],
    ) -> Result<ValueRef, ExternError> {
        Ok(make::bool(true, span("extern-func")))
    }

    fn checkpoint(&self) -> u64 {
        0
    }

    fn clear(&mut self) {}
}

fn extern_spec() -> [sl::Def; 2] {
    let function = Spanned::new(
        sl::DefKind::ExternDecD((
            id("wrong_func"),
            Vec::new(),
            Vec::new(),
            make_type::text_type(),
            Vec::new(),
        )),
        span("extern-function"),
    );
    let relation_signature = (
        Spanned::new(
            Mixfix::Seq(vec![Mixfix::Arg(make_type::text_type())]),
            span("extern-signature"),
        ),
        Vec::new(),
    );
    let relation = Spanned::new(
        sl::DefKind::ExternRelD((id("wrong_rel"), relation_signature, Vec::new(), Vec::new())),
        span("extern-relation"),
    );
    [function, relation]
}

#[test]
fn guard_checks_extern_function_and_relation_outputs() {
    let mut guarded = Interpreter::new(
        &extern_spec(),
        Options {
            cache: false,
            deterministic: false,
            guard: true,
        },
        NullInterface,
        WrongTypeExtern,
    )
    .unwrap();
    assert!(guarded.eval_func("wrong_func", &[], &[]).is_err());
    assert!(guarded.eval_rel("wrong_rel", &[]).is_err());

    let mut unchecked = Interpreter::new(
        &extern_spec(),
        Options {
            cache: false,
            deterministic: false,
            guard: false,
        },
        NullInterface,
        WrongTypeExtern,
    )
    .unwrap();
    assert_eq!(
        get::bool(&unchecked.eval_func("wrong_func", &[], &[]).unwrap()),
        Ok(true)
    );
    assert_eq!(
        get::bool(&unchecked.eval_rel("wrong_rel", &[]).unwrap()[0]),
        Ok(true)
    );
}
