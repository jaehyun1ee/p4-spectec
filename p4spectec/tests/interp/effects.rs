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
