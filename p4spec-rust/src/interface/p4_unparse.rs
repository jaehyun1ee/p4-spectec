use std::collections::HashMap;

use thiserror::Error;

use crate::{
    domain::{
        atom::Atom,
        mixfix::{Mixfix, Mixop},
    },
    lang::{
        el::ast as el,
        il::ast::{DefTypKind, TypKind},
        sl::ast::{Def, DefKind},
    },
    runtime::value::{Value, ValueKind},
};

// Hint environment

type CaseId = (String, Mixop);
type HintEnv = HashMap<CaseId, Alter>;

const HINT_ID: &str = "print";

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
    fn init(exp: &el::Exp) -> Self {
        match &exp.node {
            el::ExpKind::TextE(text) => Self::Text(text.clone()),
            el::ExpKind::AtomE(atom) => Self::Atom(atom.clone()),
            el::ExpKind::SeqE(exps) => Self::Seq(exps.iter().map(Self::init).collect()),
            el::ExpKind::BrackE(left, exp, right) => {
                Self::Brack(left.clone(), Box::new(Self::init(exp)), right.clone())
            }
            el::ExpKind::HoleE(hole @ (el::Hole::Next | el::Hole::Num(_))) => {
                Self::Hole(hole.clone())
            }
            el::ExpKind::FuseE(left, right) => {
                Self::Fuse(Box::new(Self::init(left)), Box::new(Self::init(right)))
            }
            _ => Self::Other(exp.clone()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct P4Unparser {
    hints: HintEnv,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum P4UnparseError {
    #[error("cannot unparse runtime value kind {0}")]
    UnsupportedValue(&'static str),
    #[error("print hint argument {index} is out of bounds for {len} value(s)")]
    HintArgumentOutOfBounds { index: usize, len: usize },
}

impl P4Unparser {
    pub fn from_sl_spec(spec: &[Def]) -> Self {
        let mut hints = HintEnv::new();
        for definition in spec {
            let DefKind::TypD(type_id, _type_params, def_type, _hints) = &definition.node else {
                continue;
            };
            let DefTypKind::VariantT(type_cases) = &def_type.node else {
                continue;
            };
            for (not_type, _origin, case_hints) in type_cases {
                let Some(hint) = case_hints.iter().find(|hint| hint.hintid.node == HINT_ID) else {
                    continue;
                };
                hints.insert(
                    (type_id.node.clone(), not_type.node.to_mixop()),
                    Alter::init(&hint.hintexp),
                );
            }
        }
        Self { hints }
    }

    pub fn render(&self, value: &Value) -> Result<String, P4UnparseError> {
        match &value.kind {
            ValueKind::BoolV(value) => Ok(value.to_string()),
            ValueKind::NumV(value) => Ok(match value {
                crate::lang::xl::num::T::Nat(value) | crate::lang::xl::num::T::Int(value) => {
                    value.to_string()
                }
            }),
            ValueKind::TextV(value) => Ok(escape_text(value)),
            ValueKind::StructV(_) => Err(P4UnparseError::UnsupportedValue("StructV")),
            ValueKind::CaseV(value_case) => self.render_case(&value.ty, value_case),
            ValueKind::TupleV(values) => Ok(format!(
                "({})",
                values
                    .iter()
                    .map(|value| self.render(value))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ")
            )),
            ValueKind::OptV(Some(value)) => self.render(value),
            ValueKind::OptV(None) => Ok(String::new()),
            ValueKind::ListV(values) => Ok(values
                .iter()
                .map(|value| self.render(value))
                .collect::<Result<Vec<_>, _>>()?
                .join(" ")),
            ValueKind::FuncV(_) => Err(P4UnparseError::UnsupportedValue("FuncV")),
            ValueKind::ExternV(_) => Err(P4UnparseError::UnsupportedValue("ExternV")),
        }
    }

    fn render_case(
        &self,
        typ: &TypKind,
        value_case: &Mixfix<crate::runtime::value::ValueRef>,
    ) -> Result<String, P4UnparseError> {
        let (mixop, values) = value_case.split();
        let hint = match typ {
            TypKind::VarT(type_id, _) => self.hints.get(&(type_id.node.clone(), mixop)),
            _ => None,
        };
        if let Some(hint) = hint {
            return self.render_hint(hint, &values);
        }
        let rendered = values
            .iter()
            .map(|value| self.render(value))
            .collect::<Result<Vec<_>, _>>()?;
        let mut rendered = rendered.into_iter();
        Ok(value_case.render(
            |atom| render_atom(&atom.node),
            |_value| rendered.next().expect("case arguments were rendered"),
        ))
    }

    fn render_hint(
        &self,
        hint: &Alter,
        values: &[&crate::runtime::value::ValueRef],
    ) -> Result<String, P4UnparseError> {
        fn go(
            unparser: &P4Unparser,
            hint: &Alter,
            values: &[&crate::runtime::value::ValueRef],
            cursor: &mut usize,
        ) -> Result<Option<String>, P4UnparseError> {
            match hint {
                Alter::Text(text) => Ok((!text.is_empty()).then(|| text.clone())),
                Alter::Atom(atom) => Ok(Some(render_atom(&atom.node))),
                Alter::Seq(hints) => Ok(Some(
                    hints
                        .iter()
                        .map(|hint| go(unparser, hint, values, cursor))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_iter()
                        .map(Option::unwrap_or_default)
                        .collect::<Vec<_>>()
                        .join(" "),
                )),
                Alter::Brack(left, hint, right) => {
                    let middle = go(unparser, hint, values, cursor)?;
                    Ok(Some(
                        [
                            Some(render_atom(&left.node)),
                            middle,
                            Some(render_atom(&right.node)),
                        ]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                        .join(" "),
                    ))
                }
                Alter::Hole(el::Hole::Next) => {
                    let index = *cursor;
                    *cursor += 1;
                    let value =
                        values
                            .get(index)
                            .ok_or(P4UnparseError::HintArgumentOutOfBounds {
                                index,
                                len: values.len(),
                            })?;
                    unparser.render(value).map(Some)
                }
                Alter::Hole(el::Hole::Num(index)) => {
                    let index = usize::try_from(*index).unwrap_or(usize::MAX);
                    let value =
                        values
                            .get(index)
                            .ok_or(P4UnparseError::HintArgumentOutOfBounds {
                                index,
                                len: values.len(),
                            })?;
                    unparser.render(value).map(Some)
                }
                Alter::Fuse(left, right) => {
                    let left = go(unparser, left, values, cursor)?.unwrap_or_default();
                    let right = go(unparser, right, values, cursor)?.unwrap_or_default();
                    Ok(Some(left + &right))
                }
                Alter::Other(exp) => Ok(Some(format!("{:?}", exp.node))),
                Alter::Hole(el::Hole::Rest | el::Hole::None) => Ok(Some(format!("{:?}", hint))),
            }
        }

        go(self, hint, values, &mut 0).map(Option::unwrap_or_default)
    }
}

fn render_atom(atom: &Atom) -> String {
    match atom {
        Atom::Tag(_) => String::new(),
        _ => atom.render().to_ascii_lowercase(),
    }
}

fn escape_text(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\u{0008}' => escaped.push_str("\\b"),
            character => escaped.push(character),
        }
    }
    escaped
}
