use super::*;
use p4spec_rust::{
    interp::{shared::error::Error, shared::eval::assign::assign_exp, sl::context::Context},
    lang::{
        common::{Variable, notation::mixfix::Mixfix},
        data::{typ, value::ValueArena},
        hints::input::InputHint,
    },
    note_phrase, phrase,
};

fn id(name: &str) -> ast::Id {
    phrase!(node: name.to_owned(), span: Span::default())
}
fn var(name: &str) -> ast::Exp {
    note_phrase!(node: ast::ExpKind::Var(id(name)), note: typ::make::nat().node, span: Span::default())
}
fn call(name: &str) -> ast::Instr {
    let exp = note_phrase!(node: ast::ExpKind::Call(id(name), vec![], vec![]), note: typ::make::nat().node, span: Span::default());
    phrase!(node: ast::InstrKind::Return(ast::ReturnInstr { exp }), span: Span::default())
}
fn fail() -> ast::Instr {
    let exp = note_phrase!(node: ast::ExpKind::Bool(false), note: typ::make::bool().node, span: Span::default());
    phrase!(node: ast::InstrKind::If(ast::IfInstr { exp, iter_exps: vec![], block: vec![], dangle: true }), span: Span::default())
}
fn func(name: &str, block: ast::Block) -> ast::Def {
    phrase!(node: ast::DefKind::MetaFunc(ast::MetaFuncDef::Defined(ast::DefinedFunc { id: id(name), tparams: vec![], params: vec![], typ: typ::make::nat(), block, block_else: None, hints: vec![] })), span: Span::default())
}
fn signature() -> ast::RelSignature {
    ast::RelSignature {
        not_typ: phrase!(node: Mixfix::Arg(typ::make::nat()), span: Span::default()),
        input_hint: InputHint::new(vec![0]),
    }
}
fn rel(name: &str, block: ast::Block) -> ast::Def {
    phrase!(node: ast::DefKind::Rel(ast::RelDef::Defined(ast::DefinedRel { id: id(name), rel_signature: signature(), exps_input: vec![var("n")], block, block_else: None, hints: vec![] })), span: Span::default())
}
fn rel_call(name: &str) -> ast::Instr {
    let instr = phrase!(node: ast::InstrKind::Result(ast::ResultInstr { rel_signature: signature(), exps: vec![] }), span: Span::default());
    phrase!(node: ast::InstrKind::Rule(ast::RuleInstr { id: id(name), not_exp: Mixfix::Arg(var("n")), input_hint: InputHint::new(vec![0]), iter_instrs: vec![], block: vec![instr] }), span: Span::default())
}
fn evaluate(spec_sl: ast::Spec, relation: bool) -> Error {
    let mut runner = Runner::new(
        Global::load(spec_sl).unwrap(),
        SlInterp::new(Config::new(false, false, true)),
        p4spec_rust::interface::p4(&vec![]),
        NullExtern,
    );
    if relation {
        let value = make::nat(runner.arena_mut(), 5u64.into(), Span::default()).unwrap();
        runner.context().call_rel("entry", &[value]).unwrap_err()
    } else {
        runner.context().call_func("entry", &[], &[]).unwrap_err()
    }
}

#[test]
fn optional_destructuring_preserves_outer_scalars() {
    let global = Global::load(vec![]).unwrap();
    let mut ctx = Context::new(&global);
    let mut arena = ValueArena::default();
    let value_outer = make::nat(&mut arena, 5u64.into(), Span::default()).unwrap();
    ctx.add_value(Variable::new(id("n"), vec![]), value_outer);
    let typ_tuple = typ::make::tuple(vec![typ::make::nat(), typ::make::nat()]);
    let exp_tuple = note_phrase!(node: ast::ExpKind::Tuple(vec![var("n"), var("m")]), note: typ_tuple.node.clone(), span: Span::default());
    let typ_opt = typ::make::opt(typ_tuple.clone());
    let vars = ["n", "m"]
        .into_iter()
        .map(|name| ast::Var { id: id(name), typ: typ::make::nat(), iters: vec![] })
        .collect();
    let exp = note_phrase!(node: ast::ExpKind::Iter(Box::new(exp_tuple), (ast::Iter::Opt, vars)), note: typ_opt.node.clone(), span: Span::default());
    let values: Vec<_> = [7u64, 9]
        .into_iter()
        .map(|num| make::nat(&mut arena, num.into(), Span::default()).unwrap())
        .collect();
    let value_tuple =
        make::tuple(&mut arena, typ_tuple.node.into(), values.clone(), Span::default()).unwrap();
    let value_opt =
        make::opt(&mut arena, typ_opt.node.clone().into(), Some(value_tuple), Span::default())
            .unwrap();
    let ctx = assign_exp(&mut arena, ctx, &exp, value_opt)
        .finish()
        .unwrap();
    assert_eq!(*ctx.find_value(&Variable::new(id("n"), vec![])).unwrap(), value_outer);
    assert!(
        ctx.find_value_opt(&Variable::new(id("m"), vec![]))
            .is_none()
    );
    for (name, value) in ["n", "m"].into_iter().zip(values) {
        let value_opt = ctx
            .find_value(&Variable::new(id(name), vec![ast::Iter::Opt]))
            .unwrap();
        assert_eq!(get::opt(&arena, value_opt).unwrap(), Some(value));
    }
    let value_none = make::opt(&mut arena, typ_opt.node.into(), None, Span::default()).unwrap();
    let ctx = assign_exp(&mut arena, ctx, &exp, value_none)
        .finish()
        .unwrap();
    assert_eq!(*ctx.find_value(&Variable::new(id("n"), vec![])).unwrap(), value_outer);
    assert!(
        ctx.find_value_opt(&Variable::new(id("m"), vec![]))
            .is_none()
    );
}

#[test]
fn function_tail_failures_retain_every_invocation() {
    for instr_leaf in [fail(), call("missing")] {
        let error = evaluate(
            vec![
                func("entry", vec![call("middle")]),
                func("middle", vec![call("leaf")]),
                func("leaf", vec![instr_leaf]),
            ],
            false,
        );
        let message = error.to_string();
        for name in ["entry", "middle", "leaf"] {
            assert!(message.contains(&format!("function ${name}")), "{message}");
        }
    }
}

#[test]
fn relation_tail_failures_retain_every_invocation() {
    for instr_leaf in [fail(), call("missing")] {
        let error = evaluate(
            vec![
                rel("entry", vec![rel_call("middle")]),
                rel("middle", vec![rel_call("leaf")]),
                rel("leaf", vec![instr_leaf]),
            ],
            true,
        );
        let message = error.to_string();
        for name in ["entry", "middle", "leaf"] {
            assert!(message.contains(&format!("relation {name}")), "{message}");
        }
    }
}

#[test]
fn sequential_fallback_retains_the_deeper_failure_tree() {
    let error = evaluate(
        vec![func("entry", vec![call("deep"), fail()]), func("deep", vec![fail()])],
        false,
    );
    assert!(error.to_string().contains("function $deep"), "{error}");
}

#[test]
fn sequential_fallback_prefers_the_later_failure_at_equal_depth() {
    use p4spec_rust::lang::common::source::Position;
    let mut instr_l = fail();
    let mut instr_r = fail();
    let span_l = Span::new(Position::new("branches", 1, 0), Position::new("branches", 1, 1));
    let span_r = Span::new(Position::new("branches", 2, 0), Position::new("branches", 2, 1));
    for (instr, span) in [(&mut instr_l, span_l.clone()), (&mut instr_r, span_r.clone())] {
        let ast::InstrKind::If(instr_if) = &mut instr.node else { unreachable!() };
        instr_if.exp.span = span;
    }
    let error = evaluate(vec![func("entry", vec![instr_l, instr_r])], false);
    let mut errors = vec![&error];
    let mut spans = Vec::new();
    while let Some(error) = errors.pop() {
        spans.push(&error.span);
        errors.extend(&error.children);
    }
    assert!(spans.contains(&&span_r));
    assert!(!spans.contains(&&span_l));
}

#[test]
fn long_tail_failures_render_and_drop_on_a_small_stack() {
    for relation in [false, true] {
        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(move || {
                let mut spec_sl = Vec::new();
                for idx in 0..20_000 {
                    let name = if idx == 0 { "entry".to_owned() } else { format!("step{idx}") };
                    let block = if idx + 1 == 20_000 {
                        vec![fail()]
                    } else {
                        let name_tail = format!("step{}", idx + 1);
                        vec![if relation { rel_call(&name_tail) } else { call(&name_tail) }]
                    };
                    spec_sl.push(if relation { rel(&name, block) } else { func(&name, block) });
                }
                let error = evaluate(spec_sl, relation);
                let mut errors = vec![&error];
                let mut count = 0;
                while let Some(error) = errors.pop() {
                    count += 1;
                    errors.extend(&error.children);
                }
                assert!(count >= 20_000, "tail frames were lost: {count}");
                let message = error.to_string();
                assert!(message.contains("entry"));
                assert!(message.contains("step19999"));
                drop(error);
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

#[test]
fn deeply_nested_blocks_execute_on_a_small_stack() {
    std::thread::Builder::new().stack_size(256 * 1024).spawn(|| {
        let exp = note_phrase!(node: ast::ExpKind::Num(p4spec_rust::lang::xl::num::Number::Nat(7u64.into())), note: typ::make::nat().node, span: Span::default());
        let mut instr = phrase!(node: ast::InstrKind::Return(ast::ReturnInstr { exp }), span: Span::default());
        for _ in 0..128 {
            instr = phrase!(node: ast::InstrKind::Group(ast::GroupInstr { id: id("group"), rel_signature: signature(), exps: vec![], block: vec![instr] }), span: Span::default());
        }
        let mut runner = Runner::new(Global::load(vec![func("entry", vec![instr])]).unwrap(), SlInterp::new(Config::new(false, false, true)), p4spec_rust::interface::p4(&vec![]), NullExtern);
        let value = runner.context().call_func("entry", &[], &[]).unwrap();
        assert_eq!(get::num(runner.arena(), &value).unwrap().to_string(), "7");
    }).unwrap().join().unwrap();
}

fn if_call(name: &str) -> ast::Instr {
    let exp = note_phrase!(node: ast::ExpKind::Call(id(name), vec![], vec![]), note: typ::make::bool().node, span: Span::default());
    phrase!(node: ast::InstrKind::If(ast::IfInstr { exp, iter_exps: vec![], block: vec![], dangle: true }), span: Span::default())
}

fn return_nat(num: u64) -> ast::Instr {
    let exp = note_phrase!(node: ast::ExpKind::Num(p4spec_rust::lang::xl::num::Number::Nat(num.into())), note: typ::make::nat().node, span: Span::default());
    phrase!(node: ast::InstrKind::Return(ast::ReturnInstr { exp }), span: Span::default())
}

#[test]
fn let_body_unmatch_continues_to_the_next_instruction() {
    let exp_zero = note_phrase!(node: ast::ExpKind::Num(p4spec_rust::lang::xl::num::Number::Nat(0u64.into())), note: typ::make::nat().node, span: Span::default());
    let instr_let = phrase!(node: ast::InstrKind::Let(ast::LetInstr {
        exp_l: var("n"), exp_r: exp_zero, iter_instrs: vec![], block: vec![if_call("miss")],
    }), span: Span::default());
    for det in [false, true] {
        let spec_sl =
            vec![func("entry", vec![instr_let.clone(), return_nat(9)]), func("miss", vec![fail()])];
        let mut runner = Runner::new(
            Global::load(spec_sl).unwrap(),
            SlInterp::new(Config::new(false, det, false)),
            p4spec_rust::interface::p4(&vec![]),
            NullExtern,
        );
        let value = runner.context().call_func("entry", &[], &[]).unwrap();
        assert_eq!(get::num(runner.arena(), &value).unwrap().to_string(), "9");
    }
}

#[test]
fn conditional_unmatch_escapes_sequential_blocks_but_not_deterministic_blocks() {
    for det in [false, true] {
        let spec_sl =
            vec![func("entry", vec![if_call("miss"), return_nat(9)]), func("miss", vec![fail()])];
        let mut runner = Runner::new(
            Global::load(spec_sl).unwrap(),
            SlInterp::new(Config::new(false, det, false)),
            p4spec_rust::interface::p4(&vec![]),
            NullExtern,
        );
        let result = runner.context().call_func("entry", &[], &[]);
        if det {
            assert_eq!(
                get::num(runner.arena(), &result.unwrap())
                    .unwrap()
                    .to_string(),
                "9"
            );
        } else {
            let error = result.unwrap_err();
            assert!(error.to_string().contains("function $miss"), "{error}");
        }
    }
}

#[test]
fn else_does_not_catch_an_unmatch_escaping_the_body() {
    let mut def_entry = func("entry", vec![if_call("miss")]);
    let ast::DefKind::MetaFunc(ast::MetaFuncDef::Defined(func_entry)) = &mut def_entry.node else {
        unreachable!()
    };
    func_entry.block_else = Some(vec![return_nat(9)]);
    let error = evaluate(vec![def_entry, func("miss", vec![fail()])], false);
    assert!(error.to_string().contains("function $miss"), "{error}");
}
