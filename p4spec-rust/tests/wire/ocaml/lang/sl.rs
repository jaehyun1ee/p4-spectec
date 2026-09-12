use p4spec_rust::{
    lang::{
        common::source::{Position, Span},
        data::typ,
        sl::ast,
    },
    note_phrase, phrase,
    wire::ocaml::lang::sl::SpecCodec,
};

#[test]
fn test_sl_wire_preserves_extern_declarations_and_instruction_annotations() {
    let span = Span::new(
        Position::new("structure.spec", 7, 3),
        Position::new("structure.spec", 7, 9),
    );
    let id = phrase!(node: "state".to_owned(), span: span.clone());
    let typ = typ::make::var(id.clone(), vec![]);
    let extern_typ = ast::TypDef::Extern(ast::ExternTyp {
        id: id.clone(),
        hints: vec![],
    });
    let extern_func = ast::MetaFuncDef::Extern(ast::ExternFunc {
        id: id.clone(),
        tparams: vec![],
        params: vec![],
        typ: typ.clone(),
        hints: vec![],
    });
    let exp =
        note_phrase!(node: ast::ExpKind::Bool(true), note: typ.node.clone(), span: span.clone());
    let instr = phrase!(node: ast::InstrKind::Return(ast::ReturnInstr { exp }), span: span.clone());
    let func = ast::MetaFuncDef::Defined(ast::DefinedFunc {
        id,
        tparams: vec![],
        params: vec![],
        typ,
        block: vec![instr],
        block_else: None,
        hints: vec![],
    });
    let spec = vec![
        phrase!(node: ast::DefKind::Typ(extern_typ), span: span.clone()),
        phrase!(node: ast::DefKind::MetaFunc(extern_func), span: span.clone()),
        phrase!(node: ast::DefKind::MetaFunc(func), span: span),
    ];
    let json = SpecCodec::encode(&spec).unwrap();
    assert_eq!(SpecCodec::decode(&json).unwrap(), spec);
    let mut json_located = json.clone();
    json_located[2]["at"]["left"]["line"] = serde_json::json!(11);
    assert_ne!(SpecCodec::decode(&json_located).unwrap(), spec);
}
