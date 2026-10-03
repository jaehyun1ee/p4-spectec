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
        _ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        _name: &str,
        _values: &[Value],
    ) -> Result<(Vec<Value>, bool), ExternError>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        Err(ExternError::diagnostic_unconfigured())
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
