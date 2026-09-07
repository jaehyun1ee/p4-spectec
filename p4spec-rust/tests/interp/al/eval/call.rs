use p4spec_rust::interp::al::error::TraceErrorKind;
use std::rc::Rc;

use p4spec_rust::lang::traits::print::Print;
use p4spec_rust::{
    frontend::parse::parse_string,
    interp::al::{Al, Config, context::Global, error::ErrorKind},
    lang::{
        al::ast,
        common::source::Span,
        data::{
            typ,
            value::{Value, get, make},
        },
    },
    pass::{algo, elaborate},
    phrase,
    runner::{BuiltinInterface, NullExtern, Runner},
};

fn spec(source: &str) -> ast::Spec {
    let spec_el = parse_string(source).expect("parse execution fixture");
    let spec_il = elaborate::elaborate(spec_el).expect("elaborate execution fixture");
    algo::convert(spec_il).expect("convert execution fixture")
}

fn runner(spec_al: ast::Spec, det: bool) -> Runner<Al, BuiltinInterface, NullExtern> {
    Runner::new(
        Global::load(spec_al).unwrap(),
        Config::new(det),
        BuiltinInterface::new(),
        NullExtern,
    )
}

fn nat(n: u64) -> Rc<Value> {
    make::nat(n.into(), Span::default())
}

fn number(value: &Value) -> String {
    get::num(value).unwrap().to_string()
}

#[test]
fn test_function_choice_preserves_order_and_counts_equal_successes() {
    for output in ["1", "2"] {
        let source = format!(
            "dec $pick() : nat\ndef $pick() = 1\ndef $pick() = {output}\ndef $pick() = 9\n  -- otherwise\n"
        );
        let spec_al = spec(&source);
        assert_eq!(
            number(
                &runner(spec_al.clone(), false)
                    .eval_func("pick", &[], &[])
                    .unwrap()
            ),
            "1"
        );
        let error = runner(spec_al, true)
            .eval_func("pick", &[], &[])
            .unwrap_err();
        assert!(matches!(
            *error.kind,
            ErrorKind::Trace(TraceErrorKind::Execution)
        ));
        assert!(error.to_string().contains("non-deterministic"), "{error}");
    }
}

#[test]
fn test_table_rows_remain_sequential_in_deterministic_mode() {
    let mut spec_al = spec("dec $pick() : nat\ndef $pick() = 1\ndef $pick() = 2");
    let ast::DefKind::MetaFunc(ast::MetaFuncDef::Defined(func)) = spec_al.remove(0).node else {
        panic!("defined function")
    };
    let table_rows = func
        .clauses
        .into_iter()
        .map(|clause| {
            phrase!(node: ast::TableRowKind {
            exps_signature: vec![], args: clause.node.args,
            exp: clause.node.expression, prems: clause.node.premises,
        }, span: clause.span)
        })
        .collect();
    // Overlapping rows cannot be produced by the table frontend
    let def = phrase!(node: ast::DefKind::MetaFunc(ast::MetaFuncDef::Table(ast::TableFunc {
        id: func.id, params: func.params, typ: func.typ, table_rows, hints: func.hints,
    })), span: Span::default());
    assert_eq!(
        number(&runner(vec![def], true).eval_func("pick", &[], &[]).unwrap()),
        "1"
    );
}

#[test]
fn test_builtin_failure_recovers_but_extern_failure_skips_else() {
    let source = r#"
builtin dec $text_to_int(text) : int
extern dec $unavailable() : int
dec $recover() : int
def $recover() = $text_to_int("invalid")
def $recover() = +7
  -- otherwise
dec $fatal() : int
def $fatal() = $unavailable()
def $fatal() = +9
  -- otherwise
"#;
    for det in [false, true] {
        let mut runner = runner(spec(source), det);
        assert_eq!(
            number(&runner.eval_func("recover", &[], &[]).unwrap()),
            "+7"
        );
        let error = runner.eval_func("fatal", &[], &[]).unwrap_err();
        assert!(matches!(
            *error.kind,
            ErrorKind::Trace(TraceErrorKind::Execution)
        ));
        let message = error.to_string();
        assert!(message.contains("unavailable"), "{message}");
        assert!(message.contains("fatal"), "{message}");
        assert!(!message.contains("recover"), "{message}");
    }
}

#[test]
fn test_higher_order_alias_is_resolved_in_the_caller() {
    let source = r#"
var n : nat
dec $add(nat) : nat
def $add(n) = $(n + 3)
dec $apply(nat, def $f(nat) : nat) : nat
def $apply(n, def $f) = $f(n)
dec $forward(nat, def $g(nat) : nat) : nat
def $forward(n, def $g) = $apply(n, def $g)
dec $entry(nat) : nat
def $entry(n) = $forward(n, def $add)
"#;
    assert_eq!(
        number(
            &runner(spec(source), true)
                .eval_func("entry", &[], &[nat(4)])
                .unwrap()
        ),
        "7"
    );
}

const RELATION: &str = r#"
var n : nat
relation Step: nat ~> nat ~> nat
  hint(input %0)
rule Step/first: n ~> $(n + 1) ~> $(n + 2)
rule Step/second: n ~> $(n + 1) ~> $(n + 2)
"#;

#[test]
fn test_relation_premises_bind_outputs_and_else_recovers_unmatched_paths() {
    let source = r#"
var n : nat
relation Step: nat ~> nat ~> nat
  hint(input %0)
rule Step/positive: n ~> $(n + 1) ~> $(n + 2)
  -- if $(n > 0)
rule Step/zero: n ~> 8 ~> 9
  -- otherwise
dec $use(nat) : nat
def $use(n) = $(n_a * 10 + n_b)
  -- Step: n ~> n_a ~> n_b
"#;
    for det in [false, true] {
        let mut runner = runner(spec(source), det);
        assert_eq!(
            number(&runner.eval_func("use", &[], &[nat(0)]).unwrap()),
            "89"
        );
        assert_eq!(
            number(&runner.eval_func("use", &[], &[nat(2)]).unwrap()),
            "34"
        );
    }
}

#[test]
fn test_relation_outputs_follow_notation_order_and_equal_paths_are_ambiguous() {
    let spec_al = spec(RELATION);
    let values = runner(spec_al.clone(), false)
        .eval_rel("Step", &[nat(5)])
        .unwrap();
    assert_eq!(
        values.iter().map(|value| number(value)).collect::<Vec<_>>(),
        ["6", "7"]
    );
    let error = runner(spec_al, true)
        .eval_rel("Step", &[nat(5)])
        .unwrap_err();
    assert!(matches!(
        *error.kind,
        ErrorKind::Trace(TraceErrorKind::Execution)
    ));
    assert!(error.to_string().contains("non-deterministic"), "{error}");
}

#[test]
fn test_relation_fatal_failure_does_not_select_else() {
    let source = r#"
var n : nat
extern relation Broken: nat ~> nat
  hint(input %0)
relation Recover: nat ~> nat
  hint(input %0)
rule Recover/primary: n ~> n_result
  -- Broken: n ~> n_result
rule Recover/fallback: n ~> 9
  -- otherwise
"#;
    for det in [false, true] {
        let error = runner(spec(source), det)
            .eval_rel("Recover", &[nat(1)])
            .unwrap_err();
        assert!(matches!(
            *error.kind,
            ErrorKind::Trace(TraceErrorKind::Execution)
        ));
        let message = error.to_string();
        assert!(message.contains("Broken"), "{message}");
        assert!(message.contains("Recover"), "{message}");
    }
}

#[test]
fn test_iterated_premises_collect_results_and_do_not_leak_failed_bindings() {
    let source = r#"
var n : nat
dec $map(nat*) : nat*
def $map(n*) = n_result*
  -- if (n_result = $(n + 1))*
dec $fallback(nat*) : nat*
def $fallback(n*) = n_result*
  -- if (n_result = $(n + 1))*
  -- if false
def $fallback(n*) = n*
  -- otherwise
"#;
    let mut spec_al = spec(source);
    for def in &mut spec_al {
        let ast::DefKind::MetaFunc(ast::MetaFuncDef::Defined(func)) = &mut def.node else {
            continue;
        };
        if func.id.node != "fallback" {
            continue;
        }
        let clause = &mut func.clauses[0];
        let ast::ArgKind::Exp(exp_l) = &clause.node.args[0].node else {
            panic!("expression argument")
        };
        // Force a branch-local overwrite before failure to expose scope leakage
        let prem = phrase!(node: ast::PremKind::Let(ast::LetPrem {
            exp_l: exp_l.as_ref().clone(), exp_r: clause.node.expression.clone(),
        }), span: clause.span.clone());
        clause.node.premises.insert(1, prem);
    }
    let mut runner = runner(spec_al, false);
    for values in [vec![], vec![nat(2), nat(4)]] {
        let value = make::list(
            &typ::make::list(typ::make::nat()),
            values.clone(),
            Span::default(),
        );
        let mapped = runner
            .eval_func("map", &[], std::slice::from_ref(&value))
            .unwrap();
        let expected = if values.is_empty() {
            vec![]
        } else {
            vec!["3", "5"]
        };
        assert_eq!(
            get::list(&mapped)
                .unwrap()
                .iter()
                .map(|v| number(v))
                .collect::<Vec<_>>(),
            expected
        );
        let fallback = runner
            .eval_func("fallback", &[], std::slice::from_ref(&value))
            .unwrap();
        assert_eq!(fallback.node, value.node);
    }
}

#[test]
fn test_optional_premises_collect_present_and_absent_bindings() {
    let source = r#"
var n : nat
dec $map_opt(nat?) : nat?
def $map_opt(n?) = n_result?
  -- if (n_result = $(n + 1))?
"#;
    let mut runner = runner(spec(source), true);
    for input in [None, Some(nat(6))] {
        let present = input.is_some();
        let value = make::opt(&typ::make::opt(typ::make::nat()), input, Span::default());
        let output = runner.eval_func("map_opt", &[], &[value]).unwrap();
        assert_eq!(
            get::opt(&output).unwrap().map(|value| number(value)),
            present.then(|| "7".to_owned())
        );
    }
}

#[test]
fn test_hold_and_not_hold_distinguish_unmatch_from_fatal_failure() {
    for external in [false, true] {
        let relation = if external {
            "extern relation Check: CHECK nat\n  hint(input %0)\n"
        } else {
            "relation Check: CHECK nat\n  hint(input %0)\nrule Check/zero: CHECK n\n  -- if n = 0\n"
        };
        let source = format!(
            r#"
var n : nat
{relation}
dec $hold(nat) : bool
def $hold(n) = true
  -- Check: CHECK n
def $hold(n) = false
  -- otherwise
dec $not_hold(nat) : bool
def $not_hold(n) = true
  -- Check: CHECK n
def $not_hold(n) = false
  -- otherwise
"#
        );
        let mut spec_al = spec(&source);
        for def in &mut spec_al {
            let ast::DefKind::MetaFunc(ast::MetaFuncDef::Defined(func)) = &mut def.node else {
                continue;
            };
            let negate = func.id.node == "not_hold";
            let prem = &mut func.clauses[0].node.premises[0];
            let (id, not_exp) = match &prem.node {
                ast::PremKind::Rule(prem) => (prem.id.clone(), prem.not_exp.clone()),
                ast::PremKind::IfHold(prem) => (prem.id.clone(), prem.not_exp.clone()),
                other => panic!("relation premise: {other:?}"),
            };
            prem.node = if negate {
                ast::PremKind::IfNotHold(ast::IfNotHoldPrem { id, not_exp })
            } else {
                ast::PremKind::IfHold(ast::IfHoldPrem { id, not_exp })
            };
        }
        for det in [false, true] {
            let mut runner = runner(spec_al.clone(), det);
            for n in [0, 1] {
                for (name, expected) in [("hold", n == 0), ("not_hold", n != 0)] {
                    let result = runner.eval_func(name, &[], &[nat(n)]);
                    if external {
                        let error = result.unwrap_err();
                        assert!(matches!(
                            *error.kind,
                            ErrorKind::Trace(TraceErrorKind::Execution)
                        ));
                        assert!(error.to_string().contains("Check"), "{error}");
                    } else {
                        assert_eq!(get::bool(&result.unwrap()).unwrap(), expected);
                    }
                }
            }
        }
    }
}

#[test]
fn test_native_function_calls_and_else_fallback() {
    let spec_al = spec(
        r#"
var n : nat
dec $inc(nat) : nat
def $inc(n) = n + 1
dec $choose(nat) : nat
def $choose(n) = 0
  -- if false
def $choose(n) = $inc(n)
  -- otherwise
"#,
    );
    assert_eq!(
        number(
            &runner(spec_al, false)
                .eval_func("choose", &[], &[nat(4)])
                .unwrap()
        ),
        "5"
    );
}

#[test]
fn test_program_evaluation_preserves_input_value() {
    let spec_al =
        spec("var n : nat\nrelation Pass: nat ~> nat\n  hint(input %0)\nrule Pass/pass: n ~> n");
    let mut runner = runner(spec_al, false);
    let program = nat(8);
    let values = runner.eval_program("Pass", program.clone()).unwrap();
    assert!(Rc::ptr_eq(&values[0], &program));
}

#[test]
fn test_native_table_skips_unmatched_rows_and_reports_no_success() {
    let source = r"
syntax choice
syntax firstChoice = A
syntax secondChoice = B
syntax thirdChoice = C
syntax choice =
  | firstChoice
  | secondChoice
  | thirdChoice
dec $reject() : bool
def $reject() = true
  -- if false
tbl dec $select(choice) : bool
tbl def $select =
  | A => true
  | B => false
  | C => $reject()
dec $first() : bool
def $first() = $select(A)
dec $later() : bool
def $later() = $select(B)
dec $none() : bool
def $none() = $select(C)
";
    for det in [false, true] {
        let mut runner = runner(spec(source), det);
        assert!(get::bool(&runner.eval_func("first", &[], &[]).unwrap()).unwrap());
        assert!(!get::bool(&runner.eval_func("later", &[], &[]).unwrap()).unwrap());
        let error = runner.eval_func("none", &[], &[]).unwrap_err();
        assert!(matches!(
            *error.kind,
            ErrorKind::Trace(TraceErrorKind::Execution)
        ));
        let message = error.to_string();
        assert!(message.contains("select"), "{message}");
        assert!(message.contains("reject"), "{message}");
    }
}

#[test]
fn test_deterministic_calls_preserve_fatal_failure_after_first_success() {
    let spec_al = spec(
        r#"
extern dec $fail() : nat
dec $choose() : nat
def $choose() = 1
def $choose() = $fail()
def $choose() = 9
  -- otherwise
var n : nat
relation Choose: nat ~> nat
  hint(input %0)
rule Choose/first: n ~> 1
rule Choose/fatal: n ~> $fail()
rule Choose/else: n ~> 9
  -- otherwise
"#,
    );
    for det in [false, true] {
        let mut runner = runner(spec_al.clone(), det);
        let func = runner.eval_func("choose", &[], &[]);
        let rel = runner.eval_rel("Choose", &[nat(0)]);
        if det {
            assert!(func.unwrap_err().to_string().contains("fail"));
            assert!(rel.unwrap_err().to_string().contains("fail"));
        } else {
            assert_eq!(number(&func.unwrap()), "1");
            assert_eq!(number(&rel.unwrap()[0]), "1");
        }
    }
}
