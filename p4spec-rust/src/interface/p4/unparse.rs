//! Render runtime values back to P4 surface syntax.
//!
//! The unparser collects `@print` hints from one specification IR, recursively
//! renders each runtime value, and applies a matching hint before falling back
//! to its mixfix shape. For example, a case carrying an infix `+` hint renders
//! its two arguments as `left + right`.

use std::{collections::HashMap, rc::Rc};

use crate::{
    lang::data::value::{Value, ValueKind},
    lang::{
        al,
        common::notation::{atom::Atom, mixfix::Mixfix, mixop::Mixop},
        hints::alter::{self, AlterationHint, Renderer},
        il::ast::{DefTypKind, TypKind},
        pl, sl,
        traits::print::Print,
        xl::num::Number,
    },
};

use super::error::P4UnparseError;

// == Unparser and errors

#[derive(Clone, Debug, Default)]
pub struct P4Unparser {
    hints: HashMap<(String, Mixop), AlterationHint>,
}

fn insert_case_hints(
    hints: &mut HashMap<(String, Mixop), AlterationHint>,
    type_id: &str,
    def_typ: &crate::lang::il::ast::DefTyp,
) {
    let DefTypKind::Variant(cases) = &def_typ.node else {
        return;
    };
    for (notation, _, case_hints) in cases {
        let Some((_, expression)) = case_hints.iter().find(|(id, _)| id.node == "print") else {
            continue;
        };
        let Some(hint) = alter::init(expression) else {
            continue;
        };
        hints.insert((type_id.to_owned(), notation.node.to_mixop()), hint);
    }
}

impl P4Unparser {
    // - Construction

    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_al_spec(spec_al: &[al::ast::Def]) -> Self {
        let mut hints = HashMap::new();
        for definition_al in spec_al {
            let al::ast::DefKind::Typ(type_definition_al) = &definition_al.node else {
                continue;
            };
            insert_case_hints(
                &mut hints,
                &type_definition_al.id.node,
                &type_definition_al.def_typ,
            );
        }
        Self { hints }
    }

    pub fn from_sl_spec(spec_sl: &[sl::ast::Def]) -> Self {
        let mut hints = HashMap::new();
        for definition_sl in spec_sl {
            let sl::ast::DefKind::Typ(type_definition_sl) = &definition_sl.node else {
                continue;
            };
            insert_case_hints(
                &mut hints,
                &type_definition_sl.id.node,
                &type_definition_sl.def_typ,
            );
        }
        Self { hints }
    }

    pub fn from_pl_spec(spec_pl: &[pl::ast::Def]) -> Self {
        let mut hints = HashMap::new();
        for definition_pl in spec_pl {
            let pl::ast::DefKind::Typ(type_definition_pl) = &definition_pl.node.node else {
                continue;
            };
            insert_case_hints(
                &mut hints,
                &type_definition_pl.id.node,
                &type_definition_pl.def_typ,
            );
        }
        Self { hints }
    }

    // - Rendering

    pub fn render(&self, value: &Value) -> Result<String, P4UnparseError> {
        match &value.node {
            ValueKind::Bool(value) => Ok(value.to_string()),
            ValueKind::Num(Number::Nat(value)) => Ok(value.to_string()),
            ValueKind::Num(Number::Int(value)) => Ok(value.to_string()),
            ValueKind::Text(value) => Ok(Self::escape_text(value)),
            ValueKind::Struct(_) => Err(P4UnparseError::UnsupportedValue("Struct")),
            ValueKind::Case(value_case) => self.render_case(&value.note, value_case),
            ValueKind::Tuple(values) => {
                let rendered = self.render_values(values, ", ")?;
                Ok(format!("({rendered})"))
            }
            ValueKind::Opt(Some(value)) => self.render(value),
            ValueKind::Opt(None) => Ok(String::new()),
            ValueKind::List(values) => self.render_values(values, " "),
            ValueKind::Func(_) => Err(P4UnparseError::UnsupportedValue("Func")),
            ValueKind::Extern(_) => Err(P4UnparseError::UnsupportedValue("Extern")),
        }
    }

    fn render_case(
        &self,
        typ: &TypKind,
        value_case: &Mixfix<Rc<Value>>,
    ) -> Result<String, P4UnparseError> {
        let (mixop, values) = value_case.split();
        if let TypKind::Var(type_id, _) = typ
            && let Some(hint) = self.hints.get(&(type_id.node.clone(), mixop))
        {
            return self.render_hint(hint, &values);
        }
        let mut rendered = Vec::new();
        self.render_mixfix(value_case, &mut rendered)?;
        Ok(rendered.join(" "))
    }

    fn render_hint(
        &self,
        hint: &AlterationHint,
        values: &[&Rc<Value>],
    ) -> Result<String, P4UnparseError> {
        let rendered = alter::alternate(hint, values, &ValueRenderer(self));
        match rendered {
            Ok(rendered) => rendered,
            Err(error) => Err(error.into()),
        }
    }

    fn render_values(
        &self,
        values: &[Rc<Value>],
        separator: &str,
    ) -> Result<String, P4UnparseError> {
        let rendered = values
            .iter()
            .map(|value| self.render(value))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rendered.join(separator))
    }

    fn escape_text(text: &str) -> String {
        text.chars().fold(String::new(), |mut escaped, character| {
            match character {
                '\\' => escaped.push_str("\\\\"),
                '"' => escaped.push_str("\\\""),
                '\n' => escaped.push_str("\\n"),
                '\r' => escaped.push_str("\\r"),
                '\t' => escaped.push_str("\\t"),
                '\u{0008}' => escaped.push_str("\\b"),
                character => escaped.push(character),
            }
            escaped
        })
    }

    fn render_mixfix(
        &self,
        mixfix: &Mixfix<Rc<Value>>,
        rendered: &mut Vec<String>,
    ) -> Result<(), P4UnparseError> {
        let renderer = ValueRenderer(self);
        match mixfix {
            Mixfix::Arg(value) => {
                let value = self.render(value)?;
                rendered.push(value);
            }
            Mixfix::Atom(atom) => {
                let rendered_atom = renderer.atom(atom)?;
                if !rendered_atom.is_empty() {
                    rendered.push(rendered_atom);
                }
            }
            Mixfix::Brack(atom_l, mixfix, atom_r) => {
                let rendered_atom_l = renderer.atom(atom_l)?;
                if !rendered_atom_l.is_empty() {
                    rendered.push(rendered_atom_l);
                }
                self.render_mixfix(mixfix, rendered)?;
                let rendered_atom_r = renderer.atom(atom_r)?;
                if !rendered_atom_r.is_empty() {
                    rendered.push(rendered_atom_r);
                }
            }
            Mixfix::Infix(mixfix_l, atom, mixfix_r) => {
                self.render_mixfix(mixfix_l, rendered)?;
                let rendered_atom = renderer.atom(atom)?;
                if !rendered_atom.is_empty() {
                    rendered.push(rendered_atom);
                }
                self.render_mixfix(mixfix_r, rendered)?;
            }
            Mixfix::Seq(mixfixes) => {
                for mixfix in mixfixes {
                    self.render_mixfix(mixfix, rendered)?;
                }
            }
        }
        Ok(())
    }
}

// == Print-hint rendering

struct ValueRenderer<'a>(&'a P4Unparser);

impl Renderer<&Rc<Value>> for ValueRenderer<'_> {
    type Output = Result<String, P4UnparseError>;

    fn empty(&self) -> Self::Output {
        Ok(String::new())
    }

    fn text(&self, text: &str) -> Option<Self::Output> {
        (!text.is_empty()).then(|| Ok(text.to_owned()))
    }

    fn atom(&self, atom: &crate::lang::el::ast::Atom) -> Self::Output {
        Ok(match &atom.node {
            Atom::Tag(_) => String::new(),
            Atom::Operator(operator) => operator.to_ascii_lowercase(),
            Atom::LAngle => "<".to_owned(),
            Atom::RAngle => ">".to_owned(),
            Atom::LParen => "(".to_owned(),
            Atom::RParen => ")".to_owned(),
            Atom::LBrack => "[".to_owned(),
            Atom::RBrack => "]".to_owned(),
            Atom::LBrace => "{".to_owned(),
            Atom::RBrace => "}".to_owned(),
            atom => Print::to_string(atom).to_ascii_lowercase(),
        })
    }

    fn join(&self, items: Vec<Self::Output>) -> Self::Output {
        let items = items.into_iter().collect::<Result<Vec<_>, _>>()?;
        Ok(items
            .into_iter()
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
            .join(" "))
    }

    fn fuse(&self, output_l: Self::Output, output_r: Self::Output) -> Self::Output {
        let output_l = output_l?;
        let output_r = output_r?;
        Ok(output_l + &output_r)
    }

    fn other(&self, exp: &crate::lang::el::ast::Exp) -> Self::Output {
        Ok(Print::to_string(exp))
    }

    fn item(&self, item: &&Rc<Value>) -> Self::Output {
        self.0.render(item)
    }
}
