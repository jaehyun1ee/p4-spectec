use std::{cell::RefCell, rc::Rc};

use p4spectec::{
    lang::{
        data::{
            typ::Typ,
            value::{Value, get},
        },
        traits::print::Print,
    },
    runner::{
        self, BuiltinInterface, Config, Extern, ExternError, Interface, Interpreter, Runner,
        RunnerContext,
    },
};

use super::support::sl_spec;

struct Recording {
    calls: Rc<RefCell<Vec<String>>>,
    fail: bool,
}

impl Extern for Recording {
    fn eval_rel<Interp, Iface>(
        &self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        name: &str,
        values: &[Value],
    ) -> Result<(Vec<Value>, bool), ExternError>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        assert_eq!(name, "Step");
        let num = get::num(ctx.arena(), &values[0])?.to_string();
        self.calls.borrow_mut().push(num.clone());
        if self.fail && num == "2" {
            return Err(ExternError::diagnostic_message("step two failed"));
        }
        Ok((vec![values[0]], true))
    }
    fn eval_func<Interp, Iface>(
        &self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        name: &str,
        _targs: &[Typ],
        values: &[Value],
    ) -> Result<(Value, bool), ExternError>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        assert_eq!(name, "step");
        let num = get::num(ctx.arena(), &values[0])?.to_string();
        self.calls.borrow_mut().push(num.clone());
        if self.fail && num == "2" {
            return Err(ExternError::diagnostic_message("step two failed"));
        }
        Ok((values[0], true))
    }
    fn clear(&mut self) {
        self.calls.borrow_mut().clear();
    }
}

/// Checks ordered effects and an early failing child through the public runner.
fn check<Interp>(
    mut runner: Runner<Interp, BuiltinInterface, Recording>,
    calls: Rc<RefCell<Vec<String>>>,
    fail: bool,
) where
    Interp: Interpreter<BuiltinInterface, Recording>,
{
    let result = runner.context().call_func("twice", &[], &[]);
    if fail {
        assert!(result.is_err());
        assert_eq!(*calls.borrow(), ["1", "2"]);
    } else {
        assert!(result.is_ok());
        assert_eq!(*calls.borrow(), ["1", "2", "3", "1", "2", "3"]);
    }
}

#[test]
fn composite_children_keep_effects_order_and_early_failure_in_sl_and_pl() {
    let spec_sl = sl_spec(
        r#"
syntax triple = C nat nat nat
extern dec $step(nat) : nat
dec $triple() : triple
def $triple() = C $step(1) $step(2) $step(3)
dec $twice() : (triple, triple)
def $twice() = ($triple(), $triple())
"#,
    );
    let spec_pl = p4spectec::pass::prosify::convert(spec_sl.clone()).unwrap();
    for fail in [false, true] {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let runner = runner::build_sl(
            spec_sl.clone(),
            Config::new(true, false, true),
            Recording { calls: calls.clone(), fail },
        )
        .unwrap();
        check(runner, calls, fail);
        let calls = Rc::new(RefCell::new(Vec::new()));
        let runner = runner::build_pl(
            spec_pl.clone(),
            Config::new(true, false, true),
            Recording { calls: calls.clone(), fail },
        )
        .unwrap();
        check(runner, calls, fail);
    }
}

/// Checks both inline and spilled call arguments through ordinary and tail calls.
fn check_call_args<Interp>(
    mut runner: Runner<Interp, BuiltinInterface, Recording>,
    calls: Rc<RefCell<Vec<String>>>,
    fail: bool,
    len: usize,
) where
    Interp: Interpreter<BuiltinInterface, Recording>,
{
    for entry in ["direct", "tails"] {
        calls.borrow_mut().clear();
        let result = runner.context().call_func(entry, &[], &[]);
        if fail {
            assert!(result.is_err());
            assert_eq!(*calls.borrow(), ["1", "2"]);
        } else {
            let value = result.unwrap();
            let ctx = runner.context();
            let values = get::tuple(ctx.arena(), &value).unwrap();
            let nums = values
                .iter()
                .map(|value| get::num(ctx.arena(), value).unwrap().to_string())
                .collect::<Vec<_>>();
            assert_eq!(nums, ["1", "1"]);
            let expected = (1..=len)
                .chain(1..=len)
                .map(|num| num.to_string())
                .collect::<Vec<_>>();
            assert_eq!(*calls.borrow(), expected);
        }
    }
}

#[test]
fn call_arguments_keep_order_and_early_failure_before_and_after_spilling() {
    for len in [3, 6] {
        let typs = vec!["nat"; len].join(", ");
        let ids = (0..len)
            .map(|idx| format!("n_{idx}"))
            .collect::<Vec<_>>()
            .join(", ");
        let args = (1..=len)
            .map(|num| format!("$step({num})"))
            .collect::<Vec<_>>()
            .join(", ");
        let spec_sl = sl_spec(&format!(
            "var n : nat\nextern dec $step(nat) : nat\ndec $choose({typs}) : nat\ndef $choose({ids}) = n_0\ndec $tail() : nat\ndef $tail() = $choose({args})\ndec $direct() : (nat, nat)\ndef $direct() = ($choose({args}), $choose({args}))\ndec $tails() : (nat, nat)\ndef $tails() = ($tail(), $tail())\n"
        ));
        let spec_pl = p4spectec::pass::prosify::convert(spec_sl.clone()).unwrap();
        for fail in [false, true] {
            let calls = Rc::new(RefCell::new(Vec::new()));
            let runner = runner::build_sl(
                spec_sl.clone(),
                Config::new(true, false, true),
                Recording { calls: calls.clone(), fail },
            )
            .unwrap();
            check_call_args(runner, calls, fail, len);
            let calls = Rc::new(RefCell::new(Vec::new()));
            let runner = runner::build_pl(
                spec_pl.clone(),
                Config::new(true, false, true),
                Recording { calls: calls.clone(), fail },
            )
            .unwrap();
            check_call_args(runner, calls, fail, len);
        }
    }
}

#[test]
fn binding_iterations_keep_effects_empty_rows_and_early_failures() {
    use p4spectec::lang::{
        common::source::Span,
        data::{typ, value::make},
    };

    let spec = sl_spec(
        r#"
var n : nat
extern dec $step(nat) : nat
extern relation Step: nat ~> nat
  hint(input %0)
dec $let_one(nat) : nat
def $let_one(n) = n_out
  -- if n_out = $step(n)
dec $rule_one(nat) : nat
def $rule_one(n) = n_out
  -- Step: n ~> n_out
dec $let_rows(nat*) : nat*
def $let_rows(n*) = n_out*
  -- (if n_out = $step(n))*
dec $rule_rows(nat*) : nat*
def $rule_rows(n*) = n_out*
  -- (Step: n ~> n_out)*
dec $let_nested(nat**) : nat**
def $let_nested(n**) = n_out**
  -- ((if n_out = $step(n))*)*
dec $rule_nested(nat**) : nat**
def $rule_nested(n**) = n_out**
  -- ((Step: n ~> n_out)*)*
"#,
    );
    for det in [false, true] {
        for fail in [false, true] {
            for (depth, empty) in [(0, false), (1, false), (1, true), (2, false), (2, true)] {
                let mut outcomes = Vec::new();
                for name in ["let", "rule"] {
                    let calls = Rc::new(RefCell::new(Vec::new()));
                    let mut runner = runner::build_sl(
                        spec.clone(),
                        Config::new(true, det, false),
                        Recording { calls: calls.clone(), fail },
                    )
                    .unwrap();
                    let arena = runner.arena_mut();
                    let values = [1_u64, 2, 3]
                        .map(|num| make::nat(arena, num.into(), Span::default()).unwrap());
                    let typ_list: Rc<_> = typ::make::list(typ::make::nat()).node.into();
                    let value = match depth {
                        0 => values[0],
                        1 => make::list(
                            arena,
                            typ_list.clone(),
                            if empty { vec![] } else { values.to_vec() },
                            Span::default(),
                        )
                        .unwrap(),
                        2 => {
                            let values = if empty {
                                vec![]
                            } else {
                                vec![
                                    make::list(arena, typ_list.clone(), vec![], Span::default())
                                        .unwrap(),
                                    make::list(
                                        arena,
                                        typ_list.clone(),
                                        values[..1].to_vec(),
                                        Span::default(),
                                    )
                                    .unwrap(),
                                    make::list(
                                        arena,
                                        typ_list.clone(),
                                        values[1..].to_vec(),
                                        Span::default(),
                                    )
                                    .unwrap(),
                                ]
                            };
                            make::list(
                                arena,
                                typ::make::list(typ::make::list(typ::make::nat()))
                                    .node
                                    .into(),
                                values,
                                Span::default(),
                            )
                            .unwrap()
                        }
                        _ => unreachable!(),
                    };
                    let suffix = ["one", "rows", "nested"][depth];
                    let entry = format!("{name}_{suffix}");
                    let text_input = runner.arena().to_string(&value);
                    let result = runner.context().call_func(&entry, &[], &[value]);
                    let failed = fail && !empty && depth > 0;
                    assert_eq!(result.is_err(), failed, "{entry} empty={empty}");
                    if let Ok(value_output) = result {
                        assert_eq!(runner.arena().to_string(&value_output), text_input);
                        if depth == 0 {
                            assert_eq!(value_output, value);
                        }
                    }
                    let expected: Vec<_> = if empty {
                        vec![]
                    } else if depth == 0 {
                        vec!["1"]
                    } else if failed {
                        vec!["1", "2"]
                    } else {
                        vec!["1", "2", "3"]
                    };
                    assert_eq!(*calls.borrow(), expected, "{entry}");
                    outcomes.push(calls.borrow().clone());
                }
                assert_eq!(outcomes[0], outcomes[1]);
            }
        }
    }
}
