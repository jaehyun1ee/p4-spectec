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
