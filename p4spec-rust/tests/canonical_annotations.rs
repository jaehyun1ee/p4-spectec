use p4spec_rust::{
    domain::source::{Region, Span, Spanned, phrase_list_region},
    lang::{il, sl},
};

#[test]
fn canonical_nodes_use_explicit_rust_annotations() {
    let span: Span = Region::for_file("spec/test.watsup");
    let id = Spanned::new("x".to_owned(), span.clone());
    assert_eq!(id.node, "x");
    assert_eq!(id.span, span);

    let value = il::ast::Value::new(
        il::ast::ValueKind::BoolV(true),
        il::ast::TypKind::BoolT,
        span.clone(),
    );
    assert!(matches!(value.kind, il::ast::ValueKind::BoolV(true)));
    assert!(matches!(value.ty, il::ast::TypKind::BoolT));
    assert_eq!(value.span, span);

    let exp = il::ast::Exp::new(
        il::ast::ExpKind::BoolE(false),
        il::ast::TypKind::BoolT,
        span.clone(),
    );
    assert!(matches!(exp.kind, il::ast::ExpKind::BoolE(false)));
    assert!(matches!(exp.ty, il::ast::TypKind::BoolT));
    assert_eq!(phrase_list_region(std::slice::from_ref(&exp)), span);

    let path = il::ast::Path::new(
        il::ast::PathKind::RootP,
        il::ast::TypKind::BoolT,
        span.clone(),
    );
    assert!(matches!(path.kind, il::ast::PathKind::RootP));
    assert!(matches!(path.ty, il::ast::TypKind::BoolT));

    let instr = sl::ast::Instr::new(sl::ast::InstrKind::ReturnI(exp), 9, span.clone());
    assert!(matches!(instr.kind, sl::ast::InstrKind::ReturnI(_)));
    assert_eq!(instr.iid, 9);
    assert_eq!(instr.span, span);
}
