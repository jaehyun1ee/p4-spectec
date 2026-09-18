//! Call-site type resolution preserves scope and simultaneous substitution

use std::{cell::RefCell, rc::Rc};

use p4spec_rust::{
    interp::{al, sl},
    lang::{
        al::ast,
        common::source::Span,
        data::{
            typ,
            value::{Value, ValueArena, make},
        },
    },
    note_phrase,
    pass::structure,
    phrase,
    runner::{Interface, InterfaceError, NullExtern, Runner},
};

struct TypeInterface(Rc<RefCell<Vec<ast::Typ>>>);

impl Interface for TypeInterface {
    fn call_builtin(
        &mut self,
        _arena: &mut ValueArena,
        _id: &ast::Id,
        targs: &[ast::Typ],
        values: &[Value],
    ) -> Result<(Value, bool), InterfaceError> {
        self.0.borrow_mut().extend_from_slice(targs);
        Ok((values[0], true))
    }

    fn clear(&mut self) {
        self.0.borrow_mut().clear();
    }
}

fn id(name: &str) -> ast::Id {
    phrase!(node: name.to_owned(), span: Span::default())
}

fn spec(nested: bool) -> ast::Spec {
    let exp_value = note_phrase!(node: ast::ExpKind::Id(id("n")), note: typ::make::nat().node, span: Span::default());
    let mut exp_body = note_phrase!(
        node: ast::ExpKind::Call(id("capture"),
            vec![typ::make::var(id("T"), vec![]), typ::make::var(id("U"), vec![]), typ::make::var(id("Alias"), vec![])],
            vec![phrase!(node: ast::ArgKind::Exp(Box::new(exp_value.clone())), span: Span::default())]),
        note: typ::make::nat().node, span: Span::default());
    if nested {
        exp_body = note_phrase!(node: ast::ExpKind::Tuple(vec![exp_body]), note: typ::make::tuple(vec![typ::make::nat()]).node, span: Span::default());
    }
    let param = phrase!(node: ast::ParamKind::Exp(typ::make::nat()), span: Span::default());
    let func = ast::DefinedFunc {
        id: id("entry"),
        tparams: vec![id("T"), id("U")],
        params: vec![param.clone()],
        typ: phrase!(node: exp_body.note.as_ref().clone(), span: Span::default()),
        clauses: vec![phrase!(node: ast::ClauseKind {
            args: vec![phrase!(node: ast::ArgKind::Exp(Box::new(exp_value)), span: Span::default())],
            exp: exp_body, prems: vec![],
        }, span: Span::default())],
        else_clause: None,
        hints: vec![],
    };
    let capture = ast::BuiltinFunc {
        id: id("capture"),
        tparams: vec![id("A"), id("B"), id("C")],
        params: vec![param],
        typ: typ::make::nat(),
        hints: vec![],
    };
    let mut spec = vec![
        phrase!(node: ast::DefKind::MetaFunc(ast::MetaFuncDef::Defined(Box::new(func))), span: Span::default()),
        phrase!(node: ast::DefKind::MetaFunc(ast::MetaFuncDef::Builtin(capture)), span: Span::default()),
    ];
    spec.push(phrase!(node: ast::DefKind::Typ(ast::TypDef::Defined(Box::new(ast::DefinedTyp {
            id: id("Alias"), tparams: vec![],
            def_typ: phrase!(node: ast::DefTypKind::Plain(typ::make::text()), span: Span::default()),
            hints: vec![],
        }))), span: Span::default()));
    spec
}

#[test]
fn call_type_arguments_use_local_simultaneous_substitution_in_al_and_sl() {
    let targs = [typ::make::var(id("U"), vec![]), typ::make::bool()];
    let targs_expect = [targs[0].clone(), targs[1].clone(), typ::make::var(id("Alias"), vec![])];
    for nested in [false, true] {
        let spec_al = spec(nested);
        let spec_sl = structure::convert(spec_al.clone(), false).unwrap();
        let targs_seen = Rc::new(RefCell::new(Vec::new()));
        let mut runner = Runner::new(
            al::context::Global::load(spec_al).unwrap(),
            al::AlInterp::new(al::Config::new(false, false, false)),
            TypeInterface(targs_seen.clone()),
            NullExtern,
        );
        let value = make::nat(runner.arena_mut(), 7u64.into(), Span::default()).unwrap();
        runner
            .context()
            .call_func("entry", &targs, &[value])
            .unwrap();
        assert_eq!(*targs_seen.borrow(), targs_expect);

        targs_seen.borrow_mut().clear();
        let mut runner = Runner::new(
            sl::context::Global::load(spec_sl).unwrap(),
            sl::SlInterp::new(sl::Config::new(false, false, false)),
            TypeInterface(targs_seen.clone()),
            NullExtern,
        );
        let value = make::nat(runner.arena_mut(), 7u64.into(), Span::default()).unwrap();
        runner
            .context()
            .call_func("entry", &targs, &[value])
            .unwrap();
        assert_eq!(*targs_seen.borrow(), targs_expect);
    }
}
