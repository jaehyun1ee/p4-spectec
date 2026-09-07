use p4spec_rust::interp::al::error::{HostErrorKind, TraceErrorKind};
use std::{cell::RefCell, rc::Rc};

use p4spec_rust::{
    interp::al::{Al, Config, context::Global},
    lang::{
        al::ast,
        common::{
            notation::{atom::Atom, mixfix::Mixfix},
            source::{Position, Span},
        },
        data::{
            typ,
            value::{Value, get, make},
        },
        il::ast::{ListPattern, OptPattern},
        xl::{bool as boolean, num},
    },
    runner::{Interface, InterfaceError, NullExtern, NullInterface, Runner},
};

fn id(name: &str) -> ast::Id {
    p4spec_rust::phrase!(node: name.to_owned(), span: Span::default())
}

fn exp(kind: ast::ExpKind, typ: ast::Typ) -> ast::Exp {
    p4spec_rust::note_phrase!(node: kind, note: Rc::new(typ.node), span: typ.span)
}

fn int(number: i64) -> ast::Exp {
    exp(
        ast::ExpKind::Num(num::Number::Int(number.into())),
        typ::make::int(),
    )
}

fn text(value: &str) -> ast::Exp {
    exp(ast::ExpKind::Text(value.to_owned()), typ::make::text())
}

fn list(values: Vec<ast::Exp>) -> ast::Exp {
    exp(
        ast::ExpKind::List(values),
        typ::make::list(typ::make::int()),
    )
}

fn function(name: &str, expression: ast::Exp) -> ast::Def {
    let typ =
        p4spec_rust::phrase!(node: expression.note.as_ref().clone(), span: expression.span.clone());
    p4spec_rust::phrase!(node: ast::DefKind::MetaFunc(ast::MetaFuncDef::Defined(Box::new(ast::DefinedFunc {
        id: id(name), tparams: vec![], params: vec![], typ,
        clauses: vec![p4spec_rust::phrase!(node: ast::ClauseKind { args: vec![], expression, premises: vec![] }, span: Span::default())],
        else_clause: None, hints: vec![],
    }))), span: Span::default())
}

fn eval(expression: ast::Exp) -> Result<Rc<Value>, p4spec_rust::interp::al::error::Error> {
    let global = Global::load(vec![function("test", expression)]).unwrap();
    let mut runner =
        Runner::<Al, _, _>::new(global, Config::new(false, false), NullInterface, NullExtern);
    runner.eval_func("test", &[], &[])
}

fn numbers(value: &Value) -> Vec<String> {
    get::list(value)
        .unwrap()
        .iter()
        .map(|value| num::to_int(get::num(value).unwrap()).to_string())
        .collect()
}

fn path(kind: ast::PathKind, typ: ast::Typ) -> ast::Path {
    p4spec_rust::note_phrase!(node: kind, note: Rc::new(typ.node), span: typ.span)
}

#[test]
fn test_nested_updates_preserve_the_original_and_surrounding_fields() {
    let atom = p4spec_rust::phrase!(node: Atom::Keyword("items".into()), span: Span::default());
    let typ_list = typ::make::list(typ::make::int());
    let typ_struct = typ::make::var(id("record"), vec![]);
    let original = exp(
        ast::ExpKind::Str(vec![(atom.clone(), list(vec![int(1), int(2), int(3)]))]),
        typ_struct.clone(),
    );
    let root = path(ast::PathKind::Root, typ_struct);
    let field = path(
        ast::PathKind::Dot(Box::new(root), atom.clone()),
        typ_list.clone(),
    );
    let index = path(
        ast::PathKind::Idx(Box::new(field), Box::new(int(1))),
        typ::make::int(),
    );
    let updated = exp(
        ast::ExpKind::Upd(
            Box::new(original.clone()),
            Box::new(index),
            Box::new(int(9)),
        ),
        typ::make::var(id("record"), vec![]),
    );
    let updated = exp(
        ast::ExpKind::Dot(Box::new(updated), atom.clone()),
        typ_list.clone(),
    );
    let original = exp(ast::ExpKind::Dot(Box::new(original), atom), typ_list);
    assert_eq!(numbers(&eval(updated).unwrap()), ["1", "9", "3"]);
    assert_eq!(numbers(&eval(original).unwrap()), ["1", "2", "3"]);
}

#[test]
fn test_slice_updates_require_equal_lengths_and_support_text() {
    let typ_text = typ::make::text();
    let root = path(ast::PathKind::Root, typ_text.clone());
    let slice = path(
        ast::PathKind::Slice(Box::new(root), Box::new(int(1)), Box::new(int(2))),
        typ_text.clone(),
    );
    let update = |replacement| {
        exp(
            ast::ExpKind::Upd(
                Box::new(text("abcd")),
                Box::new(slice.clone()),
                Box::new(text(replacement)),
            ),
            typ_text.clone(),
        )
    };
    assert_eq!(get::text(&eval(update("XY")).unwrap()).unwrap(), "aXYd");
    assert!(
        eval(update("X"))
            .unwrap_err()
            .to_string()
            .contains("slice of length 2")
    );
}

#[test]
fn test_negative_list_slice_lengths_are_empty_but_text_lengths_fail() {
    let slice = |base, typ| {
        exp(
            ast::ExpKind::Slice(Box::new(base), Box::new(int(2)), Box::new(int(-3))),
            typ,
        )
    };
    assert!(
        get::list(
            &eval(slice(
                list(vec![int(1), int(2), int(3)]),
                typ::make::list(typ::make::int())
            ))
            .unwrap()
        )
        .unwrap()
        .is_empty()
    );
    assert!(
        eval(slice(text("abc"), typ::make::text()))
            .unwrap_err()
            .to_string()
            .contains("negative")
    );
}

#[test]
fn test_text_operations_use_byte_lengths_and_reject_split_utf8() {
    let length = exp(ast::ExpKind::Len(Box::new(text("é"))), typ::make::nat());
    assert_eq!(
        num::to_int(get::num(&eval(length).unwrap()).unwrap()).to_string(),
        "2"
    );
    let slice = exp(
        ast::ExpKind::Slice(Box::new(text("éa")), Box::new(int(0)), Box::new(int(2))),
        typ::make::text(),
    );
    assert_eq!(get::text(&eval(slice).unwrap()).unwrap(), "é");
    let index = exp(
        ast::ExpKind::Idx(Box::new(text("é")), Box::new(int(0))),
        typ::make::text(),
    );
    assert!(
        eval(index)
            .unwrap_err()
            .to_string()
            .contains("UTF-8 boundaries")
    );
}

#[test]
fn test_structural_equality_ignores_nested_spans_and_type_notes() {
    let left = list(vec![int(3)]);
    let mut right = list(vec![int(3)]);
    if let ast::ExpKind::List(exps) = &mut right.node {
        exps[0].span = Span {
            left: Position {
                file: "other".into(),
                line: 4,
                column: 2,
            },
            right: Position {
                file: "other".into(),
                line: 4,
                column: 3,
            },
        };
    }
    let equal = exp(
        ast::ExpKind::Cmp(
            ast::CmpOp::Bool(boolean::CmpOp::Eq),
            ast::OpTyp::Int,
            Box::new(left.clone()),
            Box::new(right.clone()),
        ),
        typ::make::bool(),
    );
    assert!(get::bool(&eval(equal).unwrap()).unwrap());
    let membership = exp(
        ast::ExpKind::Mem(Box::new(left), Box::new(list(vec![right]))),
        typ::make::bool(),
    );
    assert!(get::bool(&eval(membership).unwrap()).unwrap());
}

#[test]
fn test_recursive_casts_and_subtype_checks() {
    let nat = typ::make::nat();
    let cast = exp(
        ast::ExpKind::DownCast(
            Box::new(typ::make::tuple(vec![
                nat.clone(),
                typ::make::list(nat.clone()),
            ])),
            Box::new(exp(
                ast::ExpKind::Tuple(vec![int(3), list(vec![int(4)])]),
                typ::make::tuple(vec![typ::make::int(), typ::make::list(typ::make::int())]),
            )),
        ),
        typ::make::tuple(vec![nat.clone(), typ::make::list(nat.clone())]),
    );
    let value = eval(cast).unwrap();
    let values = get::tuple(&value).unwrap();
    assert!(matches!(get::num(&values[0]).unwrap(), num::Number::Nat(_)));
    assert!(matches!(
        get::num(&get::list(&values[1]).unwrap()[0]).unwrap(),
        num::Number::Nat(_)
    ));
    let check = exp(
        ast::ExpKind::Sub(
            Box::new(int(-1)),
            Box::new(nat.clone()),
            Box::new(ast::Subcheck::Recurse(nat)),
        ),
        typ::make::bool(),
    );
    assert!(!get::bool(&eval(check).unwrap()).unwrap());
}

#[test]
fn test_case_and_container_patterns() {
    let atom = p4spec_rust::phrase!(node: Atom::Keyword("SomeCase".into()), span: Span::default());
    let case = Mixfix::Seq(vec![Mixfix::Atom(atom), Mixfix::Arg(int(5))]);
    let pattern = ast::Pattern::Case(Box::new(case.to_mixop()));
    let case = exp(
        ast::ExpKind::Case(Box::new(case)),
        typ::make::var(id("case"), vec![]),
    );
    let cases = [
        (case, pattern),
        (list(vec![int(1)]), ast::Pattern::List(ListPattern::Cons)),
        (
            list(vec![int(1), int(2)]),
            ast::Pattern::List(ListPattern::Fixed(2)),
        ),
        (list(vec![]), ast::Pattern::List(ListPattern::Nil)),
        (
            exp(
                ast::ExpKind::Opt(Some(Box::new(int(1)))),
                typ::make::opt(typ::make::int()),
            ),
            ast::Pattern::Opt(OptPattern::Some),
        ),
        (
            exp(ast::ExpKind::Opt(None), typ::make::opt(typ::make::int())),
            ast::Pattern::Opt(OptPattern::None),
        ),
    ];
    for (value, pattern) in cases {
        let matches = exp(
            ast::ExpKind::Match(Box::new(value), pattern),
            typ::make::bool(),
        );
        assert!(get::bool(&eval(matches).unwrap()).unwrap());
    }
}

struct RecordingInterface(Rc<RefCell<Vec<String>>>);

impl Interface for RecordingInterface {
    fn call_builtin(
        &mut self,
        id: &ast::Id,
        _targs: &[ast::Typ],
        _values: &[Rc<Value>],
    ) -> Result<(Rc<Value>, bool), InterfaceError> {
        self.0.borrow_mut().push(id.node.clone());
        Ok((make::bool(id.node == "right", Span::default()), true))
    }
    fn clear(&mut self) {
        self.0.borrow_mut().clear();
    }
}

#[test]
fn test_boolean_operators_evaluate_both_operands_in_order() {
    let call = |name| {
        exp(
            ast::ExpKind::Call(id(name), vec![], vec![]),
            typ::make::bool(),
        )
    };
    let expression = exp(
        ast::ExpKind::Bin(
            ast::BinOp::Bool(boolean::BinOp::And),
            ast::OpTyp::Bool,
            Box::new(call("left")),
            Box::new(call("right")),
        ),
        typ::make::bool(),
    );
    let mut defs = vec![function("test", expression)];
    for name in ["left", "right"] {
        defs.push(p4spec_rust::phrase!(node: ast::DefKind::MetaFunc(ast::MetaFuncDef::Builtin(ast::BuiltinFunc { id: id(name), tparams: vec![], params: vec![], typ: typ::make::bool(), hints: vec![] })), span: Span::default()));
    }
    let calls = Rc::new(RefCell::new(vec![]));
    let mut runner = Runner::<Al, _, _>::new(
        Global::load(defs).unwrap(),
        Config::new(false, false),
        RecordingInterface(calls.clone()),
        NullExtern,
    );
    assert!(!get::bool(&runner.eval_func("test", &[], &[]).unwrap()).unwrap());
    assert_eq!(*calls.borrow(), ["left", "right"]);
}

#[test]
fn test_numeric_errors_are_fatal_before_else_fallback() {
    let divide = exp(
        ast::ExpKind::Bin(
            ast::BinOp::Num(num::BinOp::Div),
            ast::OpTyp::Int,
            Box::new(int(1)),
            Box::new(int(0)),
        ),
        typ::make::int(),
    );
    let mut def = function("test", divide);
    if let ast::DefKind::MetaFunc(ast::MetaFuncDef::Defined(func)) = &mut def.node {
        func.else_clause = Some(
            p4spec_rust::phrase!(node: ast::ClauseKind { args: vec![], expression: int(42), premises: vec![] }, span: Span::default()),
        );
    }
    let mut runner = Runner::<Al, _, _>::new(
        Global::load(vec![def]).unwrap(),
        Config::new(false, false),
        NullInterface,
        NullExtern,
    );
    assert!(
        runner
            .eval_func("test", &[], &[])
            .unwrap_err()
            .to_string()
            .contains("zero divisor")
    );
}

#[test]
fn test_iteration_evaluates_each_bound_element_and_preserves_empty_options() {
    for iter in [ast::Iter::List, ast::Iter::Opt] {
        let typ_int = typ::make::int();
        let typ_iter = typ::make::iter(typ_int.clone(), iter);
        let var = ast::Var {
            id: id("x"),
            typ: typ_int.clone(),
            iters: vec![],
        };
        let exp_var = exp(ast::ExpKind::Var(id("x")), typ_int.clone());
        let signature = exp(
            ast::ExpKind::Iter(Box::new(exp_var.clone()), (iter, vec![var.clone()])),
            typ_iter.clone(),
        );
        let add = exp(
            ast::ExpKind::Bin(
                ast::BinOp::Num(num::BinOp::Add),
                ast::OpTyp::Int,
                Box::new(exp_var),
                Box::new(int(10)),
            ),
            typ_int,
        );
        let expression = exp(
            ast::ExpKind::Iter(Box::new(add), (iter, vec![var])),
            typ_iter.clone(),
        );
        let mut def = function("test", expression);
        if let ast::DefKind::MetaFunc(ast::MetaFuncDef::Defined(func)) = &mut def.node {
            func.params = vec![
                p4spec_rust::phrase!(node: ast::ParamKind::Exp(typ_iter.clone()), span: Span::default()),
            ];
            func.clauses[0].node.args = vec![
                p4spec_rust::phrase!(node: ast::ArgKind::Exp(Box::new(signature)), span: Span::default()),
            ];
        }
        let mut runner = Runner::<Al, _, _>::new(
            Global::load(vec![def]).unwrap(),
            Config::new(false, false),
            NullInterface,
            NullExtern,
        );
        let input = make::int(2.into(), Span::default());
        match iter {
            ast::Iter::List => {
                let input = make::list(
                    &typ_iter,
                    vec![input, make::int(4.into(), Span::default())],
                    Span::default(),
                );
                assert_eq!(
                    numbers(&runner.eval_func("test", &[], &[input]).unwrap()),
                    ["12", "14"]
                );
            }
            ast::Iter::Opt => {
                let input = make::opt(&typ_iter, Some(input), Span::default());
                let value = runner.eval_func("test", &[], &[input]).unwrap();
                assert_eq!(
                    num::to_int(get::num(get::opt(&value).unwrap().unwrap()).unwrap()).to_string(),
                    "12"
                );
                let input = make::opt(&typ_iter, None, Span::default());
                assert!(
                    get::opt(&runner.eval_func("test", &[], &[input]).unwrap())
                        .unwrap()
                        .is_none()
                );
            }
        }
    }
}

struct TypeInterface(Rc<RefCell<Vec<ast::Typ>>>);

impl Interface for TypeInterface {
    fn call_builtin(
        &mut self,
        _id: &ast::Id,
        targs: &[ast::Typ],
        values: &[Rc<Value>],
    ) -> Result<(Rc<Value>, bool), InterfaceError> {
        self.0.borrow_mut().extend_from_slice(targs);
        Ok((values[0].clone(), true))
    }
    fn clear(&mut self) {
        self.0.borrow_mut().clear();
    }
}

#[test]
fn test_call_arguments_substitute_local_types_and_pass_function_values() {
    let typ_t = typ::make::var(id("T"), vec![]);
    let typ_func = typ::make::func(vec![], vec![], typ::make::int());
    let call = exp(
        ast::ExpKind::Call(
            id("capture"),
            vec![typ_t],
            vec![
                p4spec_rust::phrase!(node: ast::ArgKind::Def(id("answer")), span: Span::default()),
            ],
        ),
        typ_func.clone(),
    );
    let mut outer = function("test", call);
    if let ast::DefKind::MetaFunc(ast::MetaFuncDef::Defined(func)) = &mut outer.node {
        func.tparams = vec![id("T")];
    }
    let capture = p4spec_rust::phrase!(node: ast::DefKind::MetaFunc(ast::MetaFuncDef::Builtin(ast::BuiltinFunc {
        id: id("capture"), tparams: vec![id("U")],
        params: vec![p4spec_rust::phrase!(node: ast::ParamKind::Def(id("f"), vec![], vec![], typ::make::int()), span: Span::default())],
        typ: typ_func, hints: vec![],
    })), span: Span::default());
    let seen = Rc::new(RefCell::new(vec![]));
    let mut runner = Runner::<Al, _, _>::new(
        Global::load(vec![outer, capture, function("answer", int(42))]).unwrap(),
        Config::new(false, false),
        TypeInterface(seen.clone()),
        NullExtern,
    );
    let value = runner.eval_func("test", &[typ::make::nat()], &[]).unwrap();
    assert_eq!(get::func(&value).unwrap().node, "answer");
    assert!(matches!(
        seen.borrow()[0].node,
        ast::TypKind::Num(num::Typ::Nat)
    ));
}

#[test]
fn test_index_failures_retain_the_index_expression_span() {
    use p4spec_rust::interp::al::error::{Error, ErrorKind};
    fn contains_span(traces: &[Error], span: &Span) -> bool {
        traces
            .iter()
            .any(|trace| &trace.span == span || contains_span(&trace.children, span))
    }
    let mut index = int(3);
    index.span = Span::new(
        Position::new("index.watsup", 7, 8),
        Position::new("index.watsup", 7, 9),
    );
    let span = index.span.clone();
    let expression = exp(
        ast::ExpKind::Idx(Box::new(list(vec![int(1)])), Box::new(index)),
        typ::make::int(),
    );
    let error = eval(expression).unwrap_err();
    let ErrorKind::Trace(TraceErrorKind::Execution) = *error.kind else {
        panic!("expected execution traces");
    };
    assert!(contains_span(&error.children, &span));
}

#[test]
fn test_native_nested_update_and_list_destructuring() {
    use p4spec_rust::{
        frontend::parse::parse_string,
        pass::{algo, elaborate},
    };

    let source = r#"
var n : nat
var m : nat
var nss : nat**
dec $update(nat**) : nat**
def $update(nss) = nss[[0][1] = 9]
dec $sum_pair(nat*) : nat
def $sum_pair([n, m]) = n + m
dec $updated() : nat**
def $updated() = $update([[1, 2], [3, 4]])
dec $destructure() : nat
def $destructure() = $sum_pair(($updated())[0])
"#;
    let spec_el = parse_string(source).unwrap();
    let spec_il = elaborate::elaborate(spec_el).unwrap();
    let spec_al = algo::convert(spec_il).unwrap();
    let mut runner = Runner::<Al, _, _>::new(
        Global::load(spec_al).unwrap(),
        Config::new(false, false),
        NullInterface,
        NullExtern,
    );
    let value = runner.eval_func("updated", &[], &[]).unwrap();
    let rows = get::list(&value).unwrap();
    assert_eq!(numbers(&rows[0]), ["1", "9"]);
    assert_eq!(numbers(&rows[1]), ["3", "4"]);
    let value = runner.eval_func("destructure", &[], &[]).unwrap();
    assert_eq!(num::to_int(get::num(&value).unwrap()).to_string(), "10");
}

#[test]
fn test_generated_values_have_default_spans_and_access_preserves_input_spans() {
    use p4spec_rust::{
        frontend::parse::parse_string,
        pass::{algo, elaborate},
    };

    let source = r#"
var n : nat
var ns : nat*
dec $literal() : nat
def $literal() = 5
dec $increment(nat) : nat
def $increment(n) = n + 1
dec $identity(nat) : nat
def $identity(n) = n
dec $first(nat*) : nat
def $first(ns) = ns[0]
"#;
    let spec_el = parse_string(source).unwrap();
    let spec_il = elaborate::elaborate(spec_el).unwrap();
    let spec_al = algo::convert(spec_il).unwrap();
    let mut runner = Runner::<Al, _, _>::new(
        Global::load(spec_al).unwrap(),
        Config::new(false, false),
        NullInterface,
        NullExtern,
    );
    let span = Span::new(
        Position::new("input.watsup", 3, 4),
        Position::new("input.watsup", 3, 5),
    );
    let input = make::nat(7u64.into(), span.clone());
    let literal = runner.eval_func("literal", &[], &[]).unwrap();
    assert_eq!(literal.span, Span::default());
    let increment = runner
        .eval_func("increment", &[], std::slice::from_ref(&input))
        .unwrap();
    assert_eq!(increment.span, Span::default());
    let identity = runner
        .eval_func("identity", &[], std::slice::from_ref(&input))
        .unwrap();
    assert_eq!(identity.span, span);
    assert!(Rc::ptr_eq(&identity, &input));
    let inputs = make::list(
        &typ::make::list(typ::make::nat()),
        vec![input.clone()],
        Span::default(),
    );
    let first = runner.eval_func("first", &[], &[inputs]).unwrap();
    assert_eq!(first.span, span);
    assert!(Rc::ptr_eq(&first, &input));
}

#[test]
fn test_builtin_failure_remains_typed_in_public_error_tree() {
    use p4spec_rust::{
        interface::builtin::error::BuiltinErrorKind,
        interp::al::error::{Error, ErrorKind},
        runner::BuiltinInterface,
    };
    fn find_builtin(error: &Error) -> Option<&BuiltinErrorKind> {
        if let ErrorKind::Host(HostErrorKind::Interface(InterfaceError::Builtin(error))) =
            error.kind.as_ref()
        {
            return Some(&error.kind);
        }
        error.children.iter().find_map(find_builtin)
    }
    let builtin = p4spec_rust::phrase!(node: ast::DefKind::MetaFunc(ast::MetaFuncDef::Builtin(ast::BuiltinFunc {
        id: id("missing_builtin"), tparams: vec![], params: vec![],
        typ: typ::make::int(), hints: vec![],
    })), span: Span::default());
    let call = exp(
        ast::ExpKind::Call(id("missing_builtin"), vec![], vec![]),
        typ::make::int(),
    );
    let mut runner = Runner::<Al, _, _>::new(
        Global::load(vec![function("test", call), builtin]).unwrap(),
        Config::new(false, false),
        BuiltinInterface::new(p4spec_rust::interface::p4::unparse::P4Unparser::new()),
        NullExtern,
    );
    let error = runner.eval_func("test", &[], &[]).unwrap_err();
    assert_eq!(
        find_builtin(&error),
        Some(&BuiltinErrorKind::MissingImplementation(
            "missing_builtin".into()
        ))
    );
}
