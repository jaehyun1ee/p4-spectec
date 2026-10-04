use std::rc::Rc;

use p4spectec::{
    interp::{
        shared::context::{ReadContext, WriteContext},
        sl::context::{Context, Global, Scope},
    },
    lang::{
        common::source::{FileId, Position, Span},
        data::typ,
        traits::eq::SyntaxEq,
    },
    phrase,
    runtime::typdef::TypeDef,
};

use super::support::sl_spec;

#[test]
fn global_names_ignore_query_spans_and_local_types_shadow_globals() {
    let global = Global::load(sl_spec(
        r#"
syntax choice = A
extern dec $first(nat) : nat
extern dec $second(bool) : bool
"#,
    ))
    .unwrap();
    let mut ctx = Context::new(&global);
    let span = Span::new(
        Position::new(FileId::intern("query.watsup"), 3, 1),
        Position::new(FileId::intern("query.watsup"), 3, 4),
    );
    let id = phrase!(node: Rc::from("first"), span: span);
    let id_default = phrase!(node: Rc::from("first"), span: Span::default());
    assert!(Rc::ptr_eq(ctx.find_func(&id).unwrap(), ctx.find_func(&id_default).unwrap()));
    let func = Rc::clone(ctx.find_func(&id).unwrap());
    assert!(ctx.add_func(id.clone(), Rc::clone(&func)).is_err());
    let id_alias = phrase!(node: Rc::from("alias"), span: span);
    ctx.add_func(id_alias.clone(), func).unwrap();
    assert_eq!(ctx.find_func_with_scope(&id_alias).unwrap().0, Scope::Local);
    assert_eq!(ctx.find_func_with_scope(&id).unwrap().0, Scope::Global);
    let id_typ = phrase!(node: Rc::from("choice"), span: span);
    assert!(ctx.find_typdef_local_opt(&id_typ).is_none());
    assert!(ctx.find_typdef_opt(&id_typ).is_some());
    let typ = typ::make::nat();
    let def_typ =
        phrase!(node: p4spectec::lang::il::ast::DefTypKind::Plain(typ.clone()), span: span);
    ctx.add_typdef_local(id_typ.clone(), TypeDef::Defined(vec![], Box::new(def_typ)))
        .unwrap();
    let (_, def_typ) = ctx.find_defined_typdef(&id_typ).unwrap();
    let p4spectec::lang::il::ast::DefTypKind::Plain(typ_local) = &def_typ.node else {
        panic!("plain local type")
    };
    assert!(typ_local.syntax_eq(&typ));
    assert!(ctx.clone().find_typdef_local_opt(&id_typ).is_some());
    assert!(ctx.localize().find_typdef_local_opt(&id_typ).is_none());
}

#[test]
fn local_bindings_keep_cloned_scopes_and_duplicate_checks_after_growth() {
    let global = Global::load(sl_spec("extern dec $identity(nat) : nat")).unwrap();
    let mut ctx = Context::new(&global);
    let span = Span::new(
        Position::new(FileId::intern("local-bindings.watsup"), 4, 1),
        Position::new(FileId::intern("local-bindings.watsup"), 4, 5),
    );
    let id_func = phrase!(node: Rc::from("identity"), span: Span::default());
    let func = Rc::clone(ctx.find_func(&id_func).unwrap());
    let bind = |ctx: &mut Context<'_>, idx| {
        let id_typ = phrase!(node: Rc::from(format!("type{idx}")), span: Span::default());
        let def_typ = phrase!(node: p4spectec::lang::il::ast::DefTypKind::Plain(typ::make::nat()), span: Span::default());
        ctx.add_typdef_local(id_typ, TypeDef::Defined(vec![], Box::new(def_typ)))
            .unwrap();
        let id_func = phrase!(node: Rc::from(format!("func{idx}")), span: Span::default());
        ctx.add_func(id_func, func.clone()).unwrap();
    };
    bind(&mut ctx, 0);
    let ctx_before = ctx.clone();
    for idx in 1..9 {
        bind(&mut ctx, idx);
    }
    for idx in 0..9 {
        let id_typ = phrase!(node: Rc::from(format!("type{idx}")), span: span);
        let id_func = phrase!(node: Rc::from(format!("func{idx}")), span: span);
        assert!(ctx.find_typdef_local_opt(&id_typ).is_some());
        assert_eq!(ctx_before.find_typdef_local_opt(&id_typ).is_some(), idx == 0);
        assert!(Rc::ptr_eq(ctx.find_func(&id_func).unwrap(), &func));
        assert_eq!(ctx_before.find_func(&id_func).is_ok(), idx == 0);
        let def_typ = phrase!(node: p4spectec::lang::il::ast::DefTypKind::Plain(typ::make::bool()), span: span);
        assert!(
            ctx.add_typdef_local(id_typ.clone(), TypeDef::Defined(vec![], Box::new(def_typ)))
                .is_err()
        );
        assert!(ctx.add_func(id_func.clone(), func.clone()).is_err());
        assert!(ctx.localize().find_typdef_local_opt(&id_typ).is_none());
        assert!(ctx.localize().find_func(&id_func).is_err());
    }
    let mut ctx_branch = ctx_before.clone();
    bind(&mut ctx_branch, 12);
    let id = phrase!(node: Rc::from("type12"), span: span);
    assert!(ctx_branch.find_typdef_local_opt(&id).is_some());
    assert!(ctx.find_typdef_local_opt(&id).is_none());
    assert!(ctx_before.find_typdef_local_opt(&id).is_none());
}
