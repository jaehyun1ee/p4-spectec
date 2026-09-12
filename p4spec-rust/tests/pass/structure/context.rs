use crate::{
    lang::{
        al::ast,
        common::notation::mixfix::Mixfix,
        il::ast::{DefTypKind, TypKind},
    },
    pass::structure::{StructureErrorKind, context::Context},
    runtime::{ops::typ::expand_typ, typdef::TypeDef},
};

use super::span;

fn id_at(text: &str, num_line: i64) -> ast::Id {
    crate::phrase! { node: text.to_owned(), span: span(num_line) }
}

fn typ_at(typ_kind: TypKind, num_line: i64) -> ast::Typ {
    crate::phrase! { node: typ_kind, span: span(num_line) }
}

fn typ_var_at(text: &str, num_line: i64) -> ast::Typ {
    typ_at(TypKind::Var(id_at(text, num_line), vec![]), num_line)
}

fn def_typ_at(def_typ_kind: DefTypKind, num_line: i64) -> ast::DefTyp {
    crate::phrase! { node: def_typ_kind, span: span(num_line) }
}

fn defined_typ(
    text: &str,
    num_line: i64,
    tparams: Vec<ast::TParam>,
    def_typ: ast::DefTyp,
) -> ast::Def {
    crate::phrase! {
        node: ast::DefKind::Typ(ast::TypDef::Defined(Box::new(ast::DefinedTyp {
            id: id_at(text, num_line),
            tparams,
            def_typ,
            hints: vec![],
        }))),
        span: span(num_line),
    }
}

fn extern_typ(text: &str, num_line: i64) -> ast::Def {
    crate::phrase! {
        node: ast::DefKind::Typ(ast::TypDef::Extern(ast::ExternTyp {
            id: id_at(text, num_line),
            hints: vec![],
        })),
        span: span(num_line),
    }
}

fn var_def(text: &str, num_line: i64, typ: ast::Typ) -> ast::Def {
    crate::phrase! {
        node: ast::DefKind::Var(ast::VarDef {
            id: id_at(text, num_line),
            typ,
            hints: vec![],
        }),
        span: span(num_line),
    }
}

#[test]
fn test_load_registers_extern_type_in_both_environments_with_declaration_span() {
    let ctx = Context::load(&vec![extern_typ("External", 5)]).expect("load extern type");
    let id_lookup = id_at("External", 99);

    assert_eq!(ctx.find_typdef(&id_lookup), Ok(&TypeDef::Extern));
    let typ_external = ctx.find_metavar(&id_lookup).expect("find extern type");
    let TypKind::Var(id_external, targs) = &typ_external.node else {
        panic!("extern type should register its named type")
    };
    assert!(targs.is_empty());
    assert_eq!(id_external.span, span(5));
    let (id_stored, _) = ctx
        .tdenv
        .iter()
        .find(|(id, _)| id.node == "External")
        .expect("stored extern type key");
    assert_eq!(id_stored.span, span(5));
    let (id_stored, _) = ctx
        .menv
        .iter()
        .find(|(id, _)| id.node == "External")
        .expect("stored extern metavariable key");
    assert_eq!(id_stored.span, span(5));
}

#[test]
fn test_load_rejects_builtin_and_declared_metavariable_collisions_at_new_span() {
    let builtin_span = span(7);
    let error = Context::load(&vec![var_def("bool", 7, typ_at(TypKind::Bool, 6))]).unwrap_err();
    assert_eq!(error.kind, StructureErrorKind::DuplicateMetavariable);
    assert_eq!(error.span, builtin_span);

    let duplicate_span = span(12);
    let spec_al = vec![
        var_def("item", 11, typ_at(TypKind::Bool, 11)),
        var_def("item", 12, typ_at(TypKind::Text, 12)),
    ];
    let error = Context::load(&spec_al).unwrap_err();
    assert_eq!(error.kind, StructureErrorKind::DuplicateMetavariable);
    assert_eq!(error.span, duplicate_span);
}

#[test]
fn test_load_rejects_duplicate_types_at_new_span() {
    let def_typ = def_typ_at(DefTypKind::Plain(typ_at(TypKind::Bool, 20)), 20);
    let spec_al = vec![
        defined_typ("Box", 20, vec![id_at("T", 20)], def_typ.clone()),
        defined_typ("Box", 21, vec![id_at("U", 21)], def_typ),
    ];

    let error = Context::load(&spec_al).unwrap_err();
    assert_eq!(error.kind, StructureErrorKind::DuplicateType);
    assert_eq!(error.span, span(21));
}

#[test]
fn test_checked_lookups_report_the_use_span() {
    let ctx = Context::load(&vec![]).expect("load empty context");
    let id_type = id_at("MissingType", 30);
    let id_metavar = id_at("missing_value", 31);

    let error = ctx.find_typdef(&id_type).unwrap_err();
    assert_eq!(error.kind, StructureErrorKind::UndefinedType);
    assert_eq!(error.span, id_type.span);

    let error = ctx.find_metavar(&id_metavar).unwrap_err();
    assert_eq!(error.kind, StructureErrorKind::UndefinedMetavariable);
    assert_eq!(error.span, id_metavar.span);
}

#[test]
fn test_load_preserves_type_keys_and_only_registers_nullary_type_metavariables() {
    let def_typ = def_typ_at(DefTypKind::Plain(typ_at(TypKind::Bool, 40)), 40);
    let spec_al = vec![
        defined_typ("Alias", 41, vec![], def_typ.clone()),
        defined_typ("Generic", 42, vec![id_at("T", 42)], def_typ),
    ];

    let ctx = Context::load(&spec_al).expect("load type definitions");
    assert!(ctx.bound_metavar(&id_at("Alias", 99)));
    assert!(!ctx.bound_metavar(&id_at("Generic", 99)));
    let (id_alias, _) = ctx
        .tdenv
        .iter()
        .find(|(id, _)| id.node == "Alias")
        .expect("stored alias key");
    assert_eq!(id_alias.span, span(41));
}

#[test]
fn test_loaded_alias_expands_to_its_variant_definition() {
    let not_typ_case = crate::phrase! {
        node: Mixfix::Arg(typ_at(TypKind::Bool, 50)),
        span: span(50),
    };
    let typ_origin = crate::phrase! {
        node: (id_at("Origin", 50), vec![]),
        span: span(50),
    };
    let def_typ_variant = def_typ_at(
        DefTypKind::Variant(vec![(not_typ_case, typ_origin, vec![])]),
        50,
    );
    let def_typ_alias = def_typ_at(DefTypKind::Plain(typ_var_at("Choice", 51)), 51);
    let spec_al = vec![
        defined_typ("Choice", 50, vec![], def_typ_variant.clone()),
        defined_typ("Alias", 51, vec![], def_typ_alias),
    ];
    let ctx = Context::load(&spec_al).expect("load alias and variant");

    let typ_alias = ctx
        .find_metavar(&id_at("Alias", 60))
        .expect("find alias metavariable");
    let typ_expanded = expand_typ(&ctx.tdenv, typ_alias).expect("expand alias");
    let TypKind::Var(id_variant, targs) = &typ_expanded.node else {
        panic!("alias should expand to a named variant")
    };
    assert!(targs.is_empty());
    assert_eq!(id_variant.node, "Choice");
    assert_eq!(
        ctx.find_typdef(id_variant)
            .expect("find variant definition"),
        &TypeDef::Defined(vec![], Box::new(def_typ_variant))
    );
}
