use std::{cell::RefCell, rc::Rc};

use p4spectec::{
    lang::{
        data::{
            typ::Typ,
            value::{Value, get},
        },
        traits::print::Print,
    },
    runner::{self, Config, Extern, ExternError, Interface, Interpreter, RunnerContext},
};

use super::support::sl_spec;

struct Recording {
    calls: Rc<RefCell<Vec<String>>>,
    effect: bool,
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
        Ok((values[0], self.effect))
    }
    fn clear(&mut self) {
        self.calls.borrow_mut().clear();
    }
}

#[test]
fn function_and_relation_calls_cache_only_pure_results_for_their_inputs() {
    let spec_sl = sl_spec(
        r#"
var n : nat
extern dec $step(nat) : nat
dec $keep(nat) : nat
def $keep(n) = $($step(n) + 0)
dec $tail(nat) : nat
def $tail(n) = $keep($(n + 1))
dec $twice() : (nat, nat, nat)
def $twice() = ($tail(1), $tail(2), $tail(1))
relation Step:
  |- nat : nat
  hint(input %0)
rule Step:
  |- n : n_out
  -- if n_out = $step($(n + 1))
dec $relations() : (nat, nat, nat)
def $relations() = (n_a, n_b, n_c)
  -- (Step: |- 1 : n_a)
  -- (Step: |- 2 : n_b)
  -- (Step: |- 1 : n_c)
"#,
    );
    for cache in [false, true] {
        for effect in [false, true] {
            let calls = Rc::new(RefCell::new(Vec::new()));
            let mut runner = runner::build_sl(
                spec_sl.clone(),
                Config::new(cache, false, true),
                Recording { calls: calls.clone(), effect },
            )
            .unwrap();
            let expected = if cache && !effect { vec!["2", "3"] } else { vec!["2", "3", "2"] };
            // A public entry clears memoized results but preserves arena handles
            for entry in ["twice", "relations", "twice", "relations"] {
                calls.borrow_mut().clear();
                let value = runner.context().call_func(entry, &[], &[]).unwrap();
                let values = get::tuple(runner.arena(), &value).unwrap();
                let nums = values
                    .iter()
                    .map(|value| get::num(runner.arena(), value).unwrap().to_string())
                    .collect::<Vec<_>>();
                assert_eq!(nums, ["2", "3", "2"]);
                assert_eq!(
                    *calls.borrow(),
                    expected,
                    "entry={entry} cache={cache} effect={effect}"
                );
            }
        }
    }
}
