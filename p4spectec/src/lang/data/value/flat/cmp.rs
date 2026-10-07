//! Comparison of flat values
//!
//! Canonical equality and hashing combine body contents and child identities.
//! `ValueRef` compares values from one arena by canonical id,
//! and resolves bodies for structural comparison across arenas.

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

use crate::lang::{
    common::prim::num,
    data::{
        intern::{CanonEq, CanonHash, CanonInterner},
        notation::{MixopArena, flat::get as get_notation},
    },
    traits::{cmp::SyntaxCmp, eq::SyntaxEq},
};

use super::{Value, ValueKind, ValueRef};

// = Canonical equality and hashing

impl CanonEq<MixopArena> for ValueKind {
    fn canon_eq(
        &self,
        interner: &CanonInterner<Self>,
        arena_mixop: &MixopArena,
        kind_r: &Self,
    ) -> bool {
        // Children compare by canonical id, computed when they were interned
        let eq_value = |value_l: &Value, value_r: &Value| {
            interner.canon_id(value_l.node) == interner.canon_id(value_r.node)
        };
        match (self, kind_r) {
            (ValueKind::Bool(value_l), ValueKind::Bool(value_r)) => value_l == value_r,
            (ValueKind::Num(value_l), ValueKind::Num(value_r)) => value_l == value_r,
            (ValueKind::Text(value_l), ValueKind::Text(value_r)) => value_l == value_r,
            (ValueKind::Struct(value_fields_l), ValueKind::Struct(value_fields_r)) => {
                value_fields_l.len() == value_fields_r.len()
                    && value_fields_l.iter().zip(value_fields_r).all(
                        |((atom_l, value_l), (atom_r, value_r))| {
                            atom_l.node == atom_r.node && eq_value(value_l, value_r)
                        },
                    )
            }
            (ValueKind::Case(value_case_l), ValueKind::Case(value_case_r)) => {
                arena_mixop.canon_eq(
                    *get_notation::mixop(value_case_l),
                    *get_notation::mixop(value_case_r),
                ) && get_notation::args(value_case_l).len()
                    == get_notation::args(value_case_r).len()
                    && get_notation::args(value_case_l)
                        .iter()
                        .zip(get_notation::args(value_case_r))
                        .all(|(value_l, value_r)| eq_value(value_l, value_r))
            }
            (ValueKind::Tuple(values_l), ValueKind::Tuple(values_r))
            | (ValueKind::List(values_l), ValueKind::List(values_r)) => {
                values_l.len() == values_r.len()
                    && values_l
                        .iter()
                        .zip(values_r)
                        .all(|(value_l, value_r)| eq_value(value_l, value_r))
            }
            (ValueKind::Opt(value_l), ValueKind::Opt(value_r)) => match (value_l, value_r) {
                (Some(value_l), Some(value_r)) => eq_value(value_l, value_r),
                (None, None) => true,
                _ => false,
            },
            (ValueKind::Func(id_l), ValueKind::Func(id_r)) => id_l.node == id_r.node,
            (ValueKind::Extern(json_l), ValueKind::Extern(json_r)) => json_l == json_r,
            _ => false,
        }
    }
}

impl CanonHash<MixopArena> for ValueKind {
    fn canon_hash<H: Hasher>(
        &self,
        interner: &CanonInterner<Self>,
        arena_mixop: &MixopArena,
        hasher: &mut H,
    ) {
        std::mem::discriminant(self).hash(hasher);
        match self {
            ValueKind::Bool(value) => value.hash(hasher),
            ValueKind::Num(value) => value.hash(hasher),
            ValueKind::Text(value) => value.hash(hasher),
            ValueKind::Struct(value_fields) => {
                value_fields.len().hash(hasher);
                for (atom, value) in value_fields {
                    atom.node.hash(hasher);
                    interner.canon_id(value.node).hash(hasher);
                }
            }
            ValueKind::Case(value_case) => {
                arena_mixop
                    .canon_id(*get_notation::mixop(value_case))
                    .hash(hasher);
                get_notation::args(value_case).len().hash(hasher);
                for value in get_notation::args(value_case) {
                    interner.canon_id(value.node).hash(hasher);
                }
            }
            ValueKind::Tuple(values) | ValueKind::List(values) => {
                values.len().hash(hasher);
                for value in values {
                    interner.canon_id(value.node).hash(hasher);
                }
            }
            ValueKind::Opt(value) => value
                .map(|value| interner.canon_id(value.node))
                .hash(hasher),
            ValueKind::Func(id) => id.node.hash(hasher),
            ValueKind::Extern(json) => json.hash(hasher),
        }
    }
}

// = Syntax comparison

impl SyntaxEq for ValueRef<'_> {
    fn syntax_eq(&self, value_other: &Self) -> bool {
        // Same arena: canonical ids decide; otherwise compare structurally
        if std::ptr::eq(self.arena, value_other.arena) {
            self.arena.canon_id(&self.value) == value_other.arena.canon_id(&value_other.value)
        } else {
            self.syntax_cmp(value_other).is_eq()
        }
    }
}

impl SyntaxCmp for ValueRef<'_> {
    fn syntax_cmp(&self, value_other: &Self) -> Ordering {
        // Children are compared through their own arenas
        let compare_value = |value_l: &Value, value_r: &Value| {
            self.arena
                .view(*value_l)
                .syntax_cmp(&value_other.arena.view(*value_r))
        };
        let compare_values = |values_l: &[Value], values_r: &[Value]| {
            values_l
                .iter()
                .zip(values_r)
                .map(|(value_l, value_r)| compare_value(value_l, value_r))
                .find(|order| !order.is_eq())
                .unwrap_or_else(|| values_l.len().cmp(&values_r.len()))
        };
        let kind_l = self.arena.kind(&self.value);
        let kind_r = value_other.arena.kind(&value_other.value);
        match (kind_l, kind_r) {
            (ValueKind::Bool(value_l), ValueKind::Bool(value_r)) => value_l.cmp(value_r),
            (ValueKind::Num(value_l), ValueKind::Num(value_r)) => num::compare(value_l, value_r),
            (ValueKind::Text(value_l), ValueKind::Text(value_r)) => value_l.cmp(value_r),
            (ValueKind::Struct(value_fields_l), ValueKind::Struct(value_fields_r)) => {
                value_fields_l
                    .iter()
                    .zip(value_fields_r)
                    .map(|((atom_l, value_l), (atom_r, value_r))| {
                        atom_l
                            .node
                            .cmp(&atom_r.node)
                            .then_with(|| compare_value(value_l, value_r))
                    })
                    .find(|order| !order.is_eq())
                    .unwrap_or_else(|| value_fields_l.len().cmp(&value_fields_r.len()))
            }
            (ValueKind::Case(value_case_l), ValueKind::Case(value_case_r)) => value_case_l.cmp_by(
                self.arena.arena_mixop(),
                value_case_r,
                value_other.arena.arena_mixop(),
                compare_value,
            ),
            (ValueKind::Tuple(values_l), ValueKind::Tuple(values_r))
            | (ValueKind::List(values_l), ValueKind::List(values_r)) => {
                compare_values(values_l, values_r)
            }
            (ValueKind::Opt(value_l), ValueKind::Opt(value_r)) => match (value_l, value_r) {
                (Some(value_l), Some(value_r)) => compare_value(value_l, value_r),
                (None, Some(_)) => Ordering::Less,
                (Some(_), None) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            },
            (ValueKind::Func(id_l), ValueKind::Func(id_r)) => id_l.node.cmp(&id_r.node),
            (ValueKind::Extern(json_l), ValueKind::Extern(json_r)) => {
                crate::util::json::compare(json_l, json_r)
            }
            // Different kinds order by tag
            _ => kind_l.tag().cmp(&kind_r.tag()),
        }
    }
}
