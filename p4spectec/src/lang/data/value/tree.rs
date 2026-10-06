//! Tree representation of values, without arena handles
//!
//! A tree contains its child values and annotations,
//! so it can be written to JSON and read into any arena.
//! Explicit conversions intern child values before their parent,
//! preserving field and argument order.

use std::{rc::Rc, slice};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::util::json::json;

use crate::lang::{
    common::{
        Id,
        notation::atom::Atom,
        prim::num::Number,
        source::{NotePhrase, Phrase},
    },
    data::{
        arena::Arena,
        notation::{AtomPhrase, Mixfix, MixopTree},
        typ::TypKind,
    },
};

use super::{
    error::ValueError,
    flat::{ValueCase, ValueFlat, ValueFlatKind},
};

/// An owned value with its type and source span.
pub type ValueTree = NotePhrase<ValueTreeKind, TypKind>;

/// A value body containing its children directly.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename = "ValueKind")]
pub enum ValueTreeKind {
    Bool(bool),
    Num(Number),
    Text(String),
    Struct(Vec<(Phrase<Atom>, ValueTree)>),
    Case(Mixfix<MixopTree, ValueTree>),
    Tuple(Vec<ValueTree>),
    Opt(Option<Box<ValueTree>>),
    List(Vec<ValueTree>),
    Func(Id),
    Extern(Rc<json>),
}

// = Arena conversion

/// Copies an arena value into a tree, including its type and source span.
pub fn from_flat(arena: &Arena, value: &ValueFlat) -> ValueTree {
    ValueTree {
        node: ValueTreeKind::from_flat(arena, arena.kind(value)),
        note: arena.typ(value).as_ref().clone(),
        span: arena.span(value).clone(),
    }
}

/// Interns the tree in the target arena, preserving its type and source span.
pub fn into_flat(arena: &mut Arena, value: ValueTree) -> Result<ValueFlat, ValueError> {
    let kind = value.node.into_flat(arena)?;
    arena.alloc(kind, value.note.into(), value.span)
}

impl ValueTreeKind {
    /// Expands child handles and case shapes into trees.
    pub(super) fn from_flat(arena: &Arena, kind: &ValueFlatKind) -> Self {
        match kind {
            ValueFlatKind::Bool(value) => Self::Bool(*value),
            ValueFlatKind::Num(num) => Self::Num(num.clone()),
            ValueFlatKind::Text(text) => Self::Text(text.clone()),
            ValueFlatKind::Struct(fields) => Self::Struct(
                fields
                    .iter()
                    .map(|(atom, value)| (atom.clone(), from_flat(arena, value)))
                    .collect(),
            ),
            ValueFlatKind::Case(value_case) => Self::Case(
                value_case
                    .to_tree(arena.arena_mixop())
                    .map_into(|value| from_flat(arena, &value)),
            ),
            ValueFlatKind::Tuple(values) => {
                Self::Tuple(values.iter().map(|value| from_flat(arena, value)).collect())
            }
            ValueFlatKind::Opt(value) => Self::Opt(
                value
                    .as_ref()
                    .map(|value| Box::new(from_flat(arena, value))),
            ),
            ValueFlatKind::List(values) => {
                Self::List(values.iter().map(|value| from_flat(arena, value)).collect())
            }
            ValueFlatKind::Func(id) => Self::Func(id.clone()),
            ValueFlatKind::Extern(json) => Self::Extern(Rc::clone(json)),
        }
    }

    /// Interns child trees in field and element order.
    pub(super) fn into_flat(self, arena: &mut Arena) -> Result<ValueFlatKind, ValueError> {
        Ok(match self {
            Self::Bool(value) => ValueFlatKind::Bool(value),
            Self::Num(num) => ValueFlatKind::Num(num),
            Self::Text(text) => ValueFlatKind::Text(text),
            Self::Struct(fields) => ValueFlatKind::Struct(
                fields
                    .into_iter()
                    .map(|(atom, value)| Ok((atom, into_flat(arena, value)?)))
                    .collect::<Result<_, ValueError>>()?,
            ),
            Self::Case(mixfix) => ValueFlatKind::Case(into_flat_case(arena, mixfix)?),
            Self::Tuple(values) => ValueFlatKind::Tuple(
                values
                    .into_iter()
                    .map(|value| into_flat(arena, value))
                    .collect::<Result<_, _>>()?,
            ),
            Self::Opt(value) => {
                ValueFlatKind::Opt(value.map(|value| into_flat(arena, *value)).transpose()?)
            }
            Self::List(values) => ValueFlatKind::List(
                values
                    .into_iter()
                    .map(|value| into_flat(arena, value))
                    .collect::<Result<_, _>>()?,
            ),
            Self::Func(id) => ValueFlatKind::Func(id),
            Self::Extern(json) => ValueFlatKind::Extern(json),
        })
    }
}

/// Interns a case's arguments in notation order, then its mixop.
fn into_flat_case(
    arena: &mut Arena,
    mixfix: Mixfix<MixopTree, ValueTree>,
) -> Result<ValueCase, ValueError> {
    let (mixop, values) = mixfix.into_parts();
    let values = values
        .into_iter()
        .map(|value| into_flat(arena, value))
        .collect::<Result<_, _>>()?;
    let arena_mixop = arena.arena_mixop_mut();
    let shape = arena_mixop.intern(&mixop)?;
    let value_case =
        ValueCase::new_in(arena_mixop, shape, values).expect("a tree case fills every position");
    Ok(value_case)
}

// = Case trees

// A case is written as its filled notation tree,
// with the variant names and shapes of the former `Mixfix`

/// A case written as its filled notation tree.
#[derive(Serialize)]
#[serde(rename = "Mixfix")]
enum CaseTree<'a, T> {
    Arg(&'a T),
    Atom(&'a AtomPhrase),
    Brack(&'a AtomPhrase, Box<CaseTree<'a, T>>, &'a AtomPhrase),
    Infix(Box<CaseTree<'a, T>>, &'a AtomPhrase, Box<CaseTree<'a, T>>),
    Seq(Vec<CaseTree<'a, T>>),
}

/// A case read as its filled notation tree.
#[derive(Deserialize)]
#[serde(rename = "Mixfix")]
enum CaseTreeOwned<T> {
    Arg(T),
    Atom(AtomPhrase),
    Brack(AtomPhrase, Box<CaseTreeOwned<T>>, AtomPhrase),
    Infix(Box<CaseTreeOwned<T>>, AtomPhrase, Box<CaseTreeOwned<T>>),
    Seq(Vec<CaseTreeOwned<T>>),
}

/// Fills a mixop with arguments in notation order, borrowing its atoms.
fn case_tree<'a, T>(mixop: &'a MixopTree, args: &mut slice::Iter<'a, T>) -> CaseTree<'a, T> {
    match mixop {
        MixopTree::Arg => CaseTree::Arg(args.next().expect("a case fills every position")),
        MixopTree::Atom(atom) => CaseTree::Atom(atom),
        MixopTree::Brack(atom_l, mixop_inner, atom_r) => {
            CaseTree::Brack(atom_l, Box::new(case_tree(mixop_inner, args)), atom_r)
        }
        MixopTree::Infix(mixop_l, atom, mixop_r) => {
            let tree_l = case_tree(mixop_l, args);
            let tree_r = case_tree(mixop_r, args);
            CaseTree::Infix(Box::new(tree_l), atom, Box::new(tree_r))
        }
        MixopTree::Seq(mixops) => {
            CaseTree::Seq(mixops.iter().map(|mixop| case_tree(mixop, args)).collect())
        }
    }
}

/// Splits a read tree into its mixop, moving arguments out in notation order.
fn split_case_tree<T>(tree: CaseTreeOwned<T>, args: &mut Vec<T>) -> MixopTree {
    match tree {
        CaseTreeOwned::Arg(arg) => {
            args.push(arg);
            MixopTree::Arg
        }
        CaseTreeOwned::Atom(atom) => MixopTree::Atom(atom),
        CaseTreeOwned::Brack(atom_l, tree, atom_r) => {
            MixopTree::Brack(atom_l, Box::new(split_case_tree(*tree, args)), atom_r)
        }
        CaseTreeOwned::Infix(tree_l, atom, tree_r) => {
            let mixop_l = split_case_tree(*tree_l, args);
            let mixop_r = split_case_tree(*tree_r, args);
            MixopTree::Infix(Box::new(mixop_l), atom, Box::new(mixop_r))
        }
        CaseTreeOwned::Seq(trees) => MixopTree::Seq(
            trees
                .into_iter()
                .map(|tree| split_case_tree(tree, args))
                .collect(),
        ),
    }
}

impl<T: Serialize> Serialize for Mixfix<MixopTree, T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        case_tree(self.mixop(), &mut self.args().iter()).serialize(serializer)
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Mixfix<MixopTree, T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut args = Vec::new();
        let mixop = split_case_tree(CaseTreeOwned::deserialize(deserializer)?, &mut args);
        Ok(Mixfix::new(mixop, args).expect("a tree case fills every position"))
    }
}
