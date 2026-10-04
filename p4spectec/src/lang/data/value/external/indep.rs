//! Arena-independent value trees
//!
//! A tree contains its child values and annotations, without arena handles,
//! so it can be written to JSON and read into any arena.

use std::rc::Rc;

use std::slice;

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
        notation::{AtomPhrase, Mixfix, Mixop, Node},
        typ::TypKind,
    },
};

use super::super::{
    Arena, Value as ArenaValue, ValueCase as ArenaValueCase, ValueError,
    ValueKind as ArenaValueKind,
};

// == Types

/// A value tree with its type; children are trees, not handles.
pub type Value = NotePhrase<ValueKind, TypKind>;

/// A value body whose children are trees.
#[derive(Debug, Serialize, Deserialize)]
pub enum ValueKind {
    Bool(bool),
    Num(Number),
    Text(String),
    Struct(Vec<(Phrase<Atom>, Value)>),
    Case(Mixfix<Mixop, Value>),
    Tuple(Vec<Value>),
    Opt(Option<Box<Value>>),
    List(Vec<Value>),
    Func(Id),
    Extern(json),
}

// == Arena conversion

/// Copies an arena value into a tree, including its type and source span.
pub fn from_arena(arena: &Arena, value: &ArenaValue) -> Value {
    Value {
        node: ValueKind::from_arena(arena, arena.kind(value)),
        note: arena.typ(value).as_ref().clone(),
        span: arena.span(value).clone(),
    }
}

/// Interns the tree in the target arena, preserving its type and source span.
pub fn into_arena(arena: &mut Arena, value: Value) -> Result<ArenaValue, ValueError> {
    let kind = value.node.into_arena(arena)?;
    arena.alloc(kind, value.note.into(), value.span)
}

impl ValueKind {
    /// Expands child handles into values and copies extern JSON unchanged.
    pub(super) fn from_arena(arena: &Arena, kind: &ArenaValueKind) -> Self {
        match kind {
            ArenaValueKind::Bool(value) => Self::Bool(*value),
            ArenaValueKind::Num(num) => Self::Num(num.clone()),
            ArenaValueKind::Text(text) => Self::Text(text.clone()),
            ArenaValueKind::Struct(fields) => Self::Struct(
                fields
                    .iter()
                    .map(|(atom, value)| (atom.clone(), from_arena(arena, value)))
                    .collect(),
            ),
            ArenaValueKind::Case(value_case) => Self::Case(
                value_case
                    .to_tree(arena.arena_shape())
                    .map_into(|value| from_arena(arena, &value)),
            ),
            ArenaValueKind::Tuple(values) => Self::Tuple(
                values
                    .iter()
                    .map(|value| from_arena(arena, value))
                    .collect(),
            ),
            ArenaValueKind::Opt(value) => Self::Opt(
                value
                    .as_ref()
                    .map(|value| Box::new(from_arena(arena, value))),
            ),
            ArenaValueKind::List(values) => Self::List(
                values
                    .iter()
                    .map(|value| from_arena(arena, value))
                    .collect(),
            ),
            ArenaValueKind::Func(id) => Self::Func(id.clone()),
            ArenaValueKind::Extern(json) => Self::Extern(json.as_ref().clone()),
        }
    }

    /// Converts child values to arena handles and keeps extern JSON unchanged.
    pub(super) fn into_arena(self, arena: &mut Arena) -> Result<ArenaValueKind, ValueError> {
        Ok(match self {
            Self::Bool(value) => ArenaValueKind::Bool(value),
            Self::Num(num) => ArenaValueKind::Num(num),
            Self::Text(text) => ArenaValueKind::Text(text),
            Self::Struct(fields) => ArenaValueKind::Struct(
                fields
                    .into_iter()
                    .map(|(atom, value)| Ok((atom, into_arena(arena, value)?)))
                    .collect::<Result<_, ValueError>>()?,
            ),
            Self::Case(mixfix) => {
                // Arguments first, then the notation's shape
                let (mixop, values) = mixfix.into_parts();
                let values = values
                    .into_iter()
                    .map(|value| into_arena(arena, value))
                    .collect::<Result<_, _>>()?;
                let arena_shape = arena.arena_shape_mut();
                let shape = arena_shape.intern(&mixop)?;
                let value_case = ArenaValueCase::new_in(arena_shape, shape, values)
                    .expect("a tree case fills every position");
                ArenaValueKind::Case(value_case)
            }
            Self::Tuple(values) => ArenaValueKind::Tuple(
                values
                    .into_iter()
                    .map(|value| into_arena(arena, value))
                    .collect::<Result<_, _>>()?,
            ),
            Self::Opt(value) => {
                ArenaValueKind::Opt(value.map(|value| into_arena(arena, *value)).transpose()?)
            }
            Self::List(values) => ArenaValueKind::List(
                values
                    .into_iter()
                    .map(|value| into_arena(arena, value))
                    .collect::<Result<_, _>>()?,
            ),
            Self::Func(id) => ArenaValueKind::Func(id),
            Self::Extern(json) => ArenaValueKind::Extern(Rc::new(json)),
        })
    }
}

// == Case trees

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
fn case_tree<'a, T>(mixop: &'a Mixop, args: &mut slice::Iter<'a, T>) -> CaseTree<'a, T> {
    match mixop {
        Node::Arg => CaseTree::Arg(args.next().expect("a case fills every position")),
        Node::Atom(atom) => CaseTree::Atom(atom),
        Node::Brack(atom_l, mixop_inner, atom_r) => {
            CaseTree::Brack(atom_l, Box::new(case_tree(mixop_inner, args)), atom_r)
        }
        Node::Infix(mixop_l, atom, mixop_r) => {
            let tree_l = case_tree(mixop_l, args);
            let tree_r = case_tree(mixop_r, args);
            CaseTree::Infix(Box::new(tree_l), atom, Box::new(tree_r))
        }
        Node::Seq(mixops) => {
            CaseTree::Seq(mixops.iter().map(|mixop| case_tree(mixop, args)).collect())
        }
    }
}

/// Splits a read tree into its mixop, moving arguments out in notation order.
fn split_case_tree<T>(tree: CaseTreeOwned<T>, args: &mut Vec<T>) -> Mixop {
    match tree {
        CaseTreeOwned::Arg(arg) => {
            args.push(arg);
            Node::Arg
        }
        CaseTreeOwned::Atom(atom) => Node::Atom(atom),
        CaseTreeOwned::Brack(atom_l, tree, atom_r) => {
            Node::Brack(atom_l, Box::new(split_case_tree(*tree, args)), atom_r)
        }
        CaseTreeOwned::Infix(tree_l, atom, tree_r) => {
            let mixop_l = split_case_tree(*tree_l, args);
            let mixop_r = split_case_tree(*tree_r, args);
            Node::Infix(Box::new(mixop_l), atom, Box::new(mixop_r))
        }
        CaseTreeOwned::Seq(trees) => Node::Seq(
            trees
                .into_iter()
                .map(|tree| split_case_tree(tree, args))
                .collect(),
        ),
    }
}

impl<T: Serialize> Serialize for Mixfix<Mixop, T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        case_tree(self.mixop(), &mut self.args().iter()).serialize(serializer)
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Mixfix<Mixop, T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut args = Vec::new();
        let mixop = split_case_tree(CaseTreeOwned::deserialize(deserializer)?, &mut args);
        Ok(Mixfix::new(mixop, args).expect("a tree case fills every position"))
    }
}
