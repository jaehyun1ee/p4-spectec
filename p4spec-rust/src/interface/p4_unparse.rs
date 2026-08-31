use std::collections::HashMap;

use thiserror::Error;

use crate::{
    lang::{
        common::notation::{atom::Atom, mixfix::Mixfix, mixop::Mixop},
        el::ast as el,
        il::ast::{DefTypKind, TypKind},
        sl::ast::{Def, DefKind},
        xl::num::Number,
    },
    runtime::value::{Value, ValueKind, ValueRef},
};

type CaseId = (String, Mixop);

#[derive(Clone, Debug)]
enum Alter {
    Text(String),
    Atom(el::Atom),
    Seq(Vec<Self>),
    Brack(el::Atom, Box<Self>, el::Atom),
    Hole(el::Hole),
    Fuse(Box<Self>, Box<Self>),
    Other(el::Exp),
}

impl Alter {
    fn from_exp(exp: &el::Exp) -> Self {
        match &exp.node {
            el::ExpKind::Text(text) => Self::Text(text.clone()),
            el::ExpKind::Atom(atom) => Self::Atom(atom.clone()),
            el::ExpKind::Seq(exps) => Self::Seq(exps.iter().map(Self::from_exp).collect()),
            el::ExpKind::Brack(left, exp, right) => {
                Self::Brack(left.clone(), Box::new(Self::from_exp(exp)), right.clone())
            }
            el::ExpKind::Hole(hole) => Self::Hole(hole.clone()),
            el::ExpKind::Fuse(left, right) => Self::Fuse(
                Box::new(Self::from_exp(left)),
                Box::new(Self::from_exp(right)),
            ),
            _ => Self::Other(exp.clone()),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct P4Unparser {
    hints: HashMap<CaseId, Alter>,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum P4UnparseError {
    #[error("cannot unparse runtime value kind {0}")]
    UnsupportedValue(&'static str),
    #[error("print hint argument {index} is out of bounds for {len} value(s)")]
    HintArgumentOutOfBounds { index: usize, len: usize },
}

impl P4Unparser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_sl_spec(spec: &[Def]) -> Self {
        let mut hints = HashMap::new();
        for definition in spec {
            let DefKind::Typ(type_definition) = &definition.node else {
                continue;
            };
            let DefTypKind::Variant(cases) = &type_definition.def_typ.node else {
                continue;
            };
            for (notation, _, case_hints) in cases {
                let Some((_, expression)) = case_hints.iter().find(|(id, _)| id.node == "print")
                else {
                    continue;
                };
                hints.insert(
                    (type_definition.id.node.clone(), notation.node.to_mixop()),
                    Alter::from_exp(expression),
                );
            }
        }
        Self { hints }
    }

    pub fn render(&self, value: &Value) -> Result<String, P4UnparseError> {
        match &value.kind {
            ValueKind::Bool(value) => Ok(value.to_string()),
            ValueKind::Num(Number::Nat(value)) => Ok(value.to_string()),
            ValueKind::Num(Number::Int(value)) => Ok(value.to_string()),
            ValueKind::Text(value) => Ok(escape_text(value)),
            ValueKind::Struct(_) => Err(P4UnparseError::UnsupportedValue("Struct")),
            ValueKind::Case(value_case) => self.render_case(&value.typ, value_case),
            ValueKind::Tuple(values) => Ok(format!("({})", self.render_values(values, ", ")?)),
            ValueKind::Opt(Some(value)) => self.render(value),
            ValueKind::Opt(None) => Ok(String::new()),
            ValueKind::List(values) => self.render_values(values, " "),
            ValueKind::Func(_) => Err(P4UnparseError::UnsupportedValue("Func")),
            ValueKind::Extern(_) => Err(P4UnparseError::UnsupportedValue("Extern")),
        }
    }

    fn render_values(
        &self,
        values: &[ValueRef],
        separator: &str,
    ) -> Result<String, P4UnparseError> {
        values
            .iter()
            .map(|value| self.render(value))
            .collect::<Result<Vec<_>, _>>()
            .map(|values| values.join(separator))
    }

    fn render_case(
        &self,
        typ: &TypKind,
        value_case: &Mixfix<ValueRef>,
    ) -> Result<String, P4UnparseError> {
        let (mixop, values) = value_case.split();
        if let TypKind::Var(type_id, _) = typ
            && let Some(hint) = self.hints.get(&(type_id.node.clone(), mixop))
        {
            return self.render_hint(hint, &values);
        }
        let rendered = values
            .iter()
            .map(|value| self.render(value))
            .collect::<Result<Vec<_>, _>>()?;
        let mut rendered = rendered.into_iter();
        Ok(value_case.render(
            |atom| render_atom(&atom.node),
            |_| rendered.next().unwrap_or_default(),
        ))
    }

    fn render_hint(&self, hint: &Alter, values: &[&ValueRef]) -> Result<String, P4UnparseError> {
        fn go(
            unparser: &P4Unparser,
            hint: &Alter,
            values: &[&ValueRef],
            cursor: &mut usize,
        ) -> Result<String, P4UnparseError> {
            match hint {
                Alter::Text(text) => Ok(text.clone()),
                Alter::Atom(atom) => Ok(render_atom(&atom.node)),
                Alter::Seq(hints) => hints
                    .iter()
                    .map(|hint| go(unparser, hint, values, cursor))
                    .collect::<Result<Vec<_>, _>>()
                    .map(|parts| {
                        parts
                            .into_iter()
                            .filter(|part| !part.is_empty())
                            .collect::<Vec<_>>()
                            .join(" ")
                    }),
                Alter::Brack(left, middle, right) => Ok([
                    render_atom(&left.node),
                    go(unparser, middle, values, cursor)?,
                    render_atom(&right.node),
                ]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" ")),
                Alter::Hole(el::Hole::Next) => {
                    let index = *cursor;
                    *cursor += 1;
                    render_at(unparser, values, index)
                }
                Alter::Hole(el::Hole::Num(index)) => render_at(
                    unparser,
                    values,
                    usize::try_from(*index).unwrap_or(usize::MAX),
                ),
                Alter::Fuse(left, right) => {
                    Ok(go(unparser, left, values, cursor)? + &go(unparser, right, values, cursor)?)
                }
                Alter::Other(expression) => Ok(format!("{:?}", expression.node)),
                Alter::Hole(el::Hole::Rest | el::Hole::None) => Ok(String::new()),
            }
        }

        go(self, hint, values, &mut 0)
    }
}

fn render_at(
    unparser: &P4Unparser,
    values: &[&ValueRef],
    index: usize,
) -> Result<String, P4UnparseError> {
    let value = values
        .get(index)
        .ok_or(P4UnparseError::HintArgumentOutOfBounds {
            index,
            len: values.len(),
        })?;
    unparser.render(value)
}

fn render_atom(atom: &Atom) -> String {
    match atom {
        Atom::Tag(_) => String::new(),
        _ => atom.render().to_ascii_lowercase(),
    }
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
