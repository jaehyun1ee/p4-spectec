use crate::lang::{
    common::{
        notation::mixfix::Mixfix,
        source::{Position, Span},
    },
    hints::input::InputHint,
    sl::ast::*,
};
use crate::pass::structure::ol::ast as ast_ol;
fn span(line: i64) -> Span {
    let pos = Position::new("structure.watsup", line, 0);
    Span::new(pos.clone(), pos)
}
fn id(text: &str) -> Id {
    crate::phrase! { node: text.to_owned(), span: span(1) }
}
fn variable(text: &str) -> Exp {
    crate::note_phrase! { node: ExpKind::Var(id(text)), note: TypKind::Bool, span: span(1) }
}
fn instr(instr_kind: ast_ol::InstrKind) -> ast_ol::Instr {
    crate::phrase! { node: instr_kind, span: span(1) }
}
fn ret(text: &str) -> ast_ol::Instr {
    instr(ast_ol::InstrKind::Return(ast_ol::ReturnInstr {
        exp: variable(text),
    }))
}
fn signature() -> RelSignature {
    RelSignature {
        not_typ: crate::phrase! { node: Mixfix::Arg(crate::phrase! {node: TypKind::Bool, span: span(1)}), span: span(1)},
        input_hint: InputHint::new(vec![0]),
    }
}
#[path = "ol.rs"]
mod ol;
#[path = "context.rs"]
mod context;
