use p4spec_rust::{
    domain::{
        mixfix::Mixfix,
        source::{Region, Spanned},
    },
    interface::{NullExtern, NullInterface},
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

fn signature() -> sl::RelSignature {
    (
        Spanned::new(Mixfix::Seq(Vec::new()), span("signature")),
        Vec::new(),
    )
}

fn variable(name: &str) -> il::Exp {
    il::Exp::new(il::ExpKind::VarE(id(name)), il::TypKind::TextT, span(name))
}

fn text(value: &str) -> il::Exp {
    il::Exp::new(
        il::ExpKind::TextE(value.to_owned()),
        il::TypKind::TextT,
        span("text"),
    )
}

fn relation(name: &str, input: il::Exp, output: il::Exp) -> sl::Def {
    Spanned::new(
        sl::DefKind::RelD((
            id(name),
            signature(),
            vec![input],
            vec![sl::Instr::new(
                sl::InstrKind::ResultI(signature(), vec![output]),
                1,
                span("result"),
            )],
            None,
            Vec::new(),
        )),
        span("relation"),
    )
}

fn function(name: &str, block: sl::Block) -> sl::Def {
    Spanned::new(
        sl::DefKind::FuncDecD((
            id(name),
            Vec::new(),
            Vec::new(),
            make_type::text_type(),
            block,
            None,
            Vec::new(),
        )),
        span("function"),
    )
}

fn rule(relation: &str, input: il::Exp, output: il::Exp, block: sl::Block, iid: i64) -> sl::Instr {
    let notation = Mixfix::Seq(vec![Mixfix::Arg(input), Mixfix::Arg(output)]);
    sl::Instr::new(
        sl::InstrKind::RuleI(id(relation), notation, vec![0], Vec::new(), block),
        iid,
        span("rule"),
    )
}

fn return_exp(exp: il::Exp, iid: i64) -> sl::Instr {
    sl::Instr::new(sl::InstrKind::ReturnI(exp), iid, span("return"))
}

fn map_identity() -> sl::Def {
    let text_type = make_type::text_type();
    let list_type = make_type::list_type(text_type.clone());
    let item = (id("item"), text_type.clone(), Vec::new());
    let output = (id("output"), text_type.clone(), Vec::new());
    let iterated = |name: &str, var: il::Var| {
        il::Exp::new(
            il::ExpKind::IterE(Box::new(variable(name)), (il::Iter::List, vec![var])),
            list_type.node.clone(),
            span("iterated"),
        )
    };
    let parameter = Spanned::new(
        sl::ParamKind::ExpP(list_type.clone(), iterated("item", item.clone())),
        span("parameter"),
    );
    let notation = Mixfix::Seq(vec![
        Mixfix::Arg(variable("item")),
        Mixfix::Arg(variable("output")),
    ]);
    let rule = sl::Instr::new(
        sl::InstrKind::RuleI(
            id("identity"),
            notation,
            vec![0],
            vec![(il::Iter::List, vec![item], vec![output.clone()])],
            vec![return_exp(iterated("output", output), 9)],
        ),
        8,
        span("rule"),
    );
    Spanned::new(
        sl::DefKind::FuncDecD((
            id("map_identity"),
            Vec::new(),
            vec![parameter],
            list_type,
            vec![rule],
            None,
            Vec::new(),
        )),
        span("map-identity"),
    )
}

fn interpreter(spec: &[sl::Def]) -> Interpreter<NullInterface, NullExtern> {
    Interpreter::new(
        spec,
        Options {
            cache: false,
            deterministic: false,
            guard: false,
        },
        NullInterface,
        NullExtern,
    )
    .unwrap()
}

#[test]
fn rule_instruction_calls_relation_assigns_outputs_and_runs_block() {
    let identity = relation("identity", variable("input"), variable("input"));
    let use_rule = function(
        "use_rule",
        vec![rule(
            "identity",
            text("value"),
            variable("output"),
            vec![return_exp(variable("output"), 3)],
            2,
        )],
    );
    let mut interpreter = interpreter(&[identity, use_rule]);

    assert_eq!(
        get::text(&interpreter.eval_func("use_rule", &[], &[]).unwrap()),
        Ok("value")
    );
}

#[test]
fn rule_unmatch_continues_to_the_next_instruction() {
    let tuple_pattern = il::Exp::new(
        il::ExpKind::TupleE(vec![variable("item")]),
        il::TypKind::TupleT(vec![make_type::text_type()]),
        span("tuple"),
    );
    let tuple_only = relation("tuple_only", tuple_pattern, text("wrong"));
    let fallback = function(
        "fallback",
        vec![
            rule(
                "tuple_only",
                text("not-a-tuple"),
                variable("output"),
                vec![return_exp(text("wrong"), 6)],
                5,
            ),
            return_exp(text("fallback"), 7),
        ],
    );
    let mut interpreter = interpreter(&[tuple_only, fallback]);

    assert_eq!(
        get::text(&interpreter.eval_func("fallback", &[], &[]).unwrap()),
        Ok("fallback")
    );
}

#[test]
fn iterated_rule_collects_relation_outputs_into_a_shared_list() {
    let identity = relation("identity", variable("input"), variable("input"));
    let mut interpreter = interpreter(&[identity, map_identity()]);
    let first = make::text("first".to_owned(), span("first"));
    let second = make::text("second".to_owned(), span("second"));
    let input = make::list(
        &make_type::list_type(make_type::text_type()),
        vec![first.clone(), second.clone()],
        span("input"),
    );

    let result = interpreter
        .eval_func("map_identity", &[], std::slice::from_ref(&input))
        .unwrap();
    let values: &[ValueRef] = get::list(&result).unwrap();
    assert!(std::rc::Rc::ptr_eq(&values[0], &first));
    assert!(std::rc::Rc::ptr_eq(&values[1], &second));

    let empty = make::list(
        &make_type::list_type(make_type::text_type()),
        Vec::new(),
        span("empty"),
    );
    let result = interpreter
        .eval_func("map_identity", &[], std::slice::from_ref(&empty))
        .unwrap();
    assert!(get::list(&result).unwrap().is_empty());
}
