use p4spec_rust::{
    lang::common::source::{Position, Span},
    lang::{
        common::notation::atom::Atom,
        el::ast::{self, ExpKind, Hole},
        hints::{
            alter::{
                self as alter_impl, AlterationError, AlterationHint, Hole as AlterHole, Renderer,
            },
            fields::{self as fields_impl, FieldError, FieldHint},
            flag as flag_impl, hint as hint_impl,
            input::{self as input_impl, InputError, InputHint},
        },
        traits::{eq::SyntaxEq, print::Print},
    },
};

fn span(s: &str) -> Span {
    Span::new(Position::new(s, 0, 0), Position::new(s, 0, 0))
}
fn atom(s: &str) -> ast::Atom {
    p4spec_rust::phrase! {
        node: Atom::Keyword(s.into()),
        span: span(s),
    }
}
fn exp(node: ExpKind) -> ast::Exp {
    p4spec_rust::phrase! {
        node: node,
        span: span("exp"),
    }
}
fn id(name: &str, source: &str) -> ast::Id {
    p4spec_rust::phrase! {
        node: name.to_owned(),
        span: span(source),
    }
}

struct StringRenderer {
    empty: &'static str,
    separator: &'static str,
    fuse: &'static str,
}

impl Renderer<&str> for StringRenderer {
    type Output = String;
    fn empty(&self) -> String {
        self.empty.into()
    }
    fn text(&self, text: &str) -> Option<String> {
        (text != "omit").then(|| text.into())
    }
    fn atom(&self, atom: &ast::Atom) -> String {
        Print::to_string(atom)
    }
    fn join(&self, items: Vec<String>) -> String {
        items.join(self.separator)
    }
    fn fuse(&self, left: String, right: String) -> String {
        format!("{left}{}{right}", self.fuse)
    }
    fn other(&self, exp: &ast::Exp) -> String {
        hint_impl::to_string(exp)
    }
    fn item(&self, item: &&str) -> String {
        (*item).into()
    }
}

#[path = "hints/alter.rs"]
mod alter;
#[path = "hints/fields.rs"]
mod fields;
#[path = "hints/flag.rs"]
mod flag;
#[path = "hints/hint.rs"]
mod hint;
#[path = "hints/input.rs"]
mod input;
